// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! What floats on a canvas beside the page: the context menu, the potential offer, the list foot and the supply
//! marks' Potential panel, each placed at its sheet point.

use super::*;

impl CanvasHost {
    /// The canvas context menu: a right-click with the Select tool opens, at the pointer, what fits the
    /// thing under it ([`ew_viewmodel::command_bar::canvas_menu`]): over an object, selected first when it is not, its
    /// own entries and then the general ones; over the title block its fields; over an empty spot Paste, the page's
    /// properties and New page (the selection cleared, so Properties shows the page). `ui.context_menu` (Shift+F10 or the
    /// menu key, the keymap's) opens the same menu at the selection while the canvas has the keys. A disabled entry shows its reason in its tooltip;
    /// a click on an entry dispatches its action and closes the menu.
    ///
    /// With any other tool a right-click is Escape (a right click cancels
    /// whatever is active, placing a device or drawing a wire): it dismisses as the key does, one step per click,
    /// and the menu opens only once the Select tool is back.
    pub(super) fn context_menu(
        &mut self,
        response: &egui::Response,
        shell: &ShellModel,
        canvas: &CanvasModel,
        group: usize,
        events: &mut Vec<Event>,
    ) {
        use ew_viewmodel::command_bar::{MenuSpot, canvas_menu, menu_spot};
        let id = response.id.with("context-menu");
        let mut open = None;
        let select = shell.tool().name() == "select";
        // The focused canvas takes the request whatever it then does with it.
        let asked = shell.focus() == Some(Focus::Canvas(group)) && std::mem::take(&mut self.menu_asked);
        let keys = asked && select && !shell.selection().items.is_empty();
        if response.secondary_clicked() && select {
            // Picked at the pointer now, not from the hover a click before cleared: of the entities a press there can
            // pick a selected one first, so a right click on what a click selected keeps it, else the hover's pick.
            let over = canvas
                .pointer_stack()
                .into_iter()
                .find(|e| shell.selection().items.contains(e))
                .or_else(|| canvas.pointer_entity().map(|(e, _)| e));
            let spot = menu_spot(canvas, over);
            let at = canvas.pointer();
            if !canvas_menu(shell, spot, at).is_empty() {
                match over.filter(|_| spot == MenuSpot::Object) {
                    Some(e) if !shell.selection().items.contains(&e) => {
                        events.push(Event::Action("select.set", json!({"ids": [e.0.to_string()]})));
                    }
                    None if !shell.selection().items.is_empty() => {
                        events.push(Event::Action("select.set", json!({"ids": []})));
                    }
                    _ => {}
                }
                self.views[group].menu_spot = spot;
                self.views[group].menu_cp = canvas.hovered_cp().map(|cp| cp.id);
                self.views[group].menu_at = None;
                self.views[group].menu_point = at;
                open = Some(egui::SetOpenCommand::Bool(true));
            }
        } else if keys {
            // At the first selected object's middle, as the pointer would stand.
            let page = canvas.page();
            self.views[group].menu_at = page
                .entities
                .iter()
                .find(|(e, _)| shell.selection().items.contains(e))
                .map(|(_, r)| ew_viewmodel::canvas::rect_centre(*r))
                .map(|(x, y)| {
                    let (px, py) = view_of(canvas).transform().to_screen((x as f64, y as f64));
                    let ppp = canvas.ppp();
                    response.rect.min + egui::vec2((px / ppp) as f32, (py / ppp) as f32)
                })
                .or(Some(response.rect.center()));
            self.views[group].menu_spot = MenuSpot::Object;
            self.views[group].menu_cp = None;
            self.views[group].menu_point = None;
            open = Some(egui::SetOpenCommand::Bool(true));
        } else if response.clicked() || response.secondary_clicked() {
            open = Some(egui::SetOpenCommand::Bool(false));
            if response.secondary_clicked() {
                events.push(Event::Action("ui.dismiss", json!({})));
            }
        }
        let spot = self.views[group].menu_spot;
        let cp = self.views[group]
            .menu_cp
            .filter(|_| spot == MenuSpot::Object)
            .and_then(|cp| ew_viewmodel::command_bar::cp_menu(shell, cp));
        let popup = egui::Popup::context_menu(response).id(id).open_memory(open);
        let popup = match self.views[group].menu_at {
            Some(at) => popup.at_position(at),
            None => popup,
        };
        popup.show(|ui| {
            // Over a connection point, Mark not used or Mark used first.
            let items: Vec<_> = cp
                .into_iter()
                .chain(canvas_menu(shell, spot, self.views[group].menu_point))
                .collect();
            crate::menu::items(ui, &items, events);
        });
    }

    /// The card of a potential reasoned for the selected wire (`ShellModel::potential_offer`): beside the
    /// wire, its name and why, Accept (`potential.offer {accept: true}`) and Decline. Hidden while a menu is open, and
    /// on any canvas but the focused one.
    pub(super) fn potential_offer(
        response: &egui::Response,
        shell: &ShellModel,
        canvas: &CanvasModel,
        group: usize,
        events: &mut Vec<Event>,
    ) {
        let focused = shell.area().is_none_or(|a| a.focused == group);
        if !focused || egui::Popup::is_any_open(&response.ctx) {
            return;
        }
        let Some((title, why)) = shell.potential_offer() else {
            return;
        };
        // Below the right end of the wire's rectangle, inside the canvas.
        let page = canvas.page();
        let Some(rect) = page
            .entities
            .iter()
            .find(|(e, _)| shell.selection().items.contains(e))
            .map(|(_, r)| *r)
        else {
            return;
        };
        let b = rect.max();
        let at = Self::screen_point(response.rect, canvas, (b.x.0 as f64, b.y.0 as f64));
        let t = crate::theme::current(&response.ctx);
        let at = at + egui::vec2(t.space_2, t.space_2);
        let at = egui::pos2(
            at.x.clamp(
                response.rect.left(),
                (response.rect.right() - t.size_dialog_label).max(response.rect.left()),
            ),
            at.y.clamp(
                response.rect.top(),
                (response.rect.bottom() - 4.0 * t.size_control).max(response.rect.top()),
            ),
        );
        egui::Area::new(response.id.with("potential-offer"))
            .order(egui::Order::Foreground)
            .fixed_pos(at)
            .show(&response.ctx, |ui| {
                crate::components::card_frame(&t).show(ui, |ui| {
                    ui.label(crate::components::body_strong(&t, &title));
                    ui.label(crate::components::small_secondary(&t, &why));
                    ui.add_space(t.space_2);
                    ui.horizontal(|ui| {
                        use crate::components::{Kind, button};
                        let accept = strings::text(key!("potential-offer-accept"));
                        if button(ui, Kind::Primary, accept, None, 0.0).clicked() {
                            events.push(Event::Action("potential.offer", json!({"accept": true})));
                        }
                        let decline = strings::text(key!("potential-offer-decline"));
                        if button(ui, Kind::Secondary, decline, None, 0.0).clicked() {
                            events.push(Event::Action("potential.offer", json!({"accept": false})));
                        }
                    });
                });
            });
    }

    /// The row under the last row of a list that runs on (`ShellModel::list_foot`: a parts list's
    /// first page as the whole list): one row of the table as wide as it, its outline in `color.accent` on
    /// `color.accent.subtle`, "The list continues on page 901.1 ›" in the table's text size, a click on it goes there.
    /// The window's own (the printed page has its title block's Next); on the focused canvas.
    pub(super) fn list_foot(
        response: &egui::Response,
        shell: &ShellModel,
        canvas: &CanvasModel,
        group: usize,
        events: &mut Vec<Event>,
    ) {
        let focused = shell.area().is_none_or(|a| a.focused == group);
        let Some((text, folio, [x0, y0, x1, y1])) = shell.list_foot().filter(|_| focused) else {
            return;
        };
        let corner = |x: i64, y: i64| Self::screen_point(response.rect, canvas, (x as f64, y as f64));
        let row = Rect::from_two_pos(corner(x0, y0), corner(x1, y1));
        if !response.rect.intersects(row) {
            return;
        }
        let t = crate::theme::current(&response.ctx);
        let id = response.id.with("list-foot");
        egui::Area::new(id)
            .order(egui::Order::Middle)
            .fixed_pos(row.min)
            .show(&response.ctx, |ui| {
                ui.set_clip_rect(response.rect);
                let r = ui.interact(row, id.with("row"), Sense::click());
                r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &text));
                let painter = ui.painter();
                let fill = if r.hovered() {
                    t.accent_subtle.color32().gamma_multiply(1.5)
                } else {
                    t.accent_subtle.color32()
                };
                painter.rect_filled(row, egui::CornerRadius::ZERO, fill);
                painter.rect_stroke(
                    row,
                    egui::CornerRadius::ZERO,
                    crate::theme::stroke(t.border_width, t.accent),
                    egui::StrokeKind::Inside,
                );
                // The table's text: about half the row's height, as the printed rows' 8.88 pt in their 6.52 mm.
                let size = (row.height() * 0.5).max(t.type_small.size);
                let font = crate::theme::font(&crate::theme::TypeToken { size, ..t.type_small });
                let galley = painter.layout_no_wrap(text.clone(), font, t.accent.color32());
                let at = egui::pos2(row.left() + size * 0.4, row.center().y - galley.size().y / 2.0);
                painter.galley(at, galley, t.accent.color32());
                if r.clicked() {
                    events.push(Event::Action("page.goto", json!({ "folio": folio })));
                }
            });
    }

    /// The supply marks' Potential panel (H9, `ShellModel::supply_panel`): below the marks it is for, a row per mark
    /// (its name, the kind of conductor, the voltage with AC or DC), Apply (Enter) and Not now (Escape). On the focused
    /// canvas only, hidden while a menu is open.
    pub(super) fn supply_panel(
        response: &egui::Response,
        shell: &ShellModel,
        canvas: &CanvasModel,
        group: usize,
        events: &mut Vec<Event>,
    ) {
        let focused = shell.area().is_none_or(|a| a.focused == group);
        if !focused || egui::Popup::is_any_open(&response.ctx) {
            return;
        }
        let Some(panel) = shell.supply_panel() else {
            return;
        };
        // Below the lowest of the marks, at the left of the leftmost, inside the canvas.
        let page = canvas.page();
        let rects: Vec<_> = page
            .entities
            .iter()
            .filter(|(e, _)| panel.rows.iter().any(|r| r.rep == e.0))
            .map(|(_, r)| *r)
            .collect();
        let t = crate::theme::current(&response.ctx);
        let at = match (
            rects.iter().map(|r| r.min().x.0).min(),
            rects.iter().map(|r| r.max().y.0).max(),
        ) {
            (Some(x), Some(y)) => {
                Self::screen_point(response.rect, canvas, (x as f64, y as f64)) + egui::vec2(0.0, t.space_2)
            }
            _ => response.rect.min + egui::vec2(t.space_4, t.space_4),
        };
        let at = egui::pos2(
            at.x.clamp(
                response.rect.left(),
                (response.rect.right() - 3.0 * t.size_dialog_label).max(response.rect.left()),
            ),
            at.y.clamp(
                response.rect.top(),
                (response.rect.bottom() - (panel.rows.len() as f32 + 4.0) * t.size_control)
                    .max(response.rect.top()),
            ),
        );
        let kinds = shell.supply_kinds();
        egui::Area::new(response.id.with("supply-panel"))
            .order(egui::Order::Foreground)
            .fixed_pos(at)
            .show(&response.ctx, |ui| {
                crate::components::card_frame(&t).show(ui, |ui| {
                    ui.label(crate::components::body_strong(
                        &t,
                        strings::text(key!("supply-potential-title")),
                    ));
                    ui.add_space(t.space_2);
                    egui::Grid::new("supply-panel-rows")
                        .num_columns(3)
                        .spacing(egui::vec2(t.space_3, t.space_2))
                        .show(ui, |ui| {
                            for k in [
                                key!("supply-potential-name"),
                                key!("supply-potential-kind"),
                                key!("supply-potential-voltage"),
                            ] {
                                ui.label(crate::components::small_secondary(&t, strings::text(k)));
                            }
                            ui.end_row();
                            for (i, row) in panel.rows.iter().enumerate() {
                                ui.label(&row.name);
                                let shown = kinds
                                    .iter()
                                    .find(|(w, _)| *w == row.kind)
                                    .map_or(row.kind.as_str(), |(_, text)| text.as_str());
                                // Each field named by its column and its row for a screen reader ("Conductor, 24V").
                                let name = |k| {
                                    strings::format(key!("form-table-cell"), &[strings::text(k), &row.name])
                                };
                                let conductor = name(key!("supply-potential-kind"));
                                let kind_choice = crate::components::choice(
                                    ui,
                                    egui::ComboBox::from_id_salt(("supply-kind", i))
                                        .selected_text(shown)
                                        .width(4.0 * t.size_control),
                                    |ui| {
                                        for (word, text) in &kinds {
                                            if ui.selectable_label(*word == row.kind, text).clicked() {
                                                events.push(Event::Action(
                                                    "ip.supply_potential",
                                                    json!({"row": i, "kind": word}),
                                                ));
                                            }
                                        }
                                    },
                                );
                                kind_choice.response.widget_info(|| {
                                    let mut info =
                                        egui::WidgetInfo::labeled(egui::Role::ComboBox, true, &conductor);
                                    info.current_text_value = Some(shown.to_owned());
                                    info
                                });
                                let mut voltage = row.voltage.clone();
                                let edit = egui::TextEdit::singleline(&mut voltage)
                                    .id_salt(("supply-voltage", i))
                                    .desired_width(5.0 * t.size_control);
                                let edit = ui.add(edit);
                                let named = name(key!("supply-potential-voltage"));
                                edit.widget_info(|| {
                                    let mut info =
                                        egui::WidgetInfo::labeled(egui::Role::TextInput, true, &named);
                                    info.current_text_value = Some(voltage.clone());
                                    info
                                });
                                if edit.changed() {
                                    events.push(Event::Action(
                                        "ip.supply_potential",
                                        json!({"row": i, "voltage": voltage}),
                                    ));
                                }
                                ui.end_row();
                            }
                        });
                    if let Some(error) = &panel.error {
                        ui.add_space(t.space_2);
                        ui.label(egui::RichText::new(error).color(t.error.color32()));
                    }
                    ui.add_space(t.space_2);
                    ui.label(crate::components::small_secondary(
                        &t,
                        strings::text(key!("supply-potential-hint")),
                    ));
                    ui.add_space(t.space_2);
                    ui.horizontal(|ui| {
                        use crate::components::{Kind, button};
                        if button(ui, Kind::Primary, strings::text(key!("dialog-apply")), None, 0.0).clicked()
                        {
                            events.push(Event::Action("ip.supply_apply", serde_json::Value::Null));
                        }
                        if button(
                            ui,
                            Kind::Secondary,
                            strings::text(key!("supply-potential-skip")),
                            None,
                            0.0,
                        )
                        .clicked()
                        {
                            events.push(Event::Action("ui.dismiss", serde_json::Value::Null));
                        }
                    });
                });
            });
    }

    /// The screen position (points) of the sheet point `(x, y)` (nm) on a canvas drawn in `rect`, with the model's
    /// exact view: where a floating card of the canvas stands.
    fn screen_point(rect: Rect, canvas: &CanvasModel, (x, y): (f64, f64)) -> Pos2 {
        let ppp = canvas.ppp();
        let (px, py) = view_of(canvas).transform().to_screen((x, y));
        rect.min + egui::vec2((px / ppp) as f32, (py / ppp) as f32)
    }
}
