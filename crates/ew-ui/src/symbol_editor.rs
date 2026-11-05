// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Symbol editor canvas and properties. All edits dispatch the shared headless actions.

use crate::{app::Event, theme};
use egui::{Pos2, Sense, Stroke, Ui, Vec2};
use ew_viewmodel::ew_api::symbols::{self, SymbolDraft};
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;
use serde_json::{Value, json};
use symbols::drawing;

#[derive(Default)]
pub(crate) struct EditorView {
    generation: Option<u64>,
    file: String,
    directory: String,
    tool: usize,
    start: Option<[f64; 2]>,
    vertices: Vec<[f64; 2]>,
    selected: Option<usize>,
    advanced: String,
    text: String,
    designation: String,
    direction: usize,
    discard: bool,
    zoom: f32,
    pan: Vec2,
    manual_view: bool,
    place: [f64; 2],
    function_key: String,
    slot_kind: usize,
    slot_pin: u16,
    link_name: String,
    json_error: bool,
}

fn label(key: strings::Key<'_>) -> &'static str {
    strings::text(key)
}
fn action(events: &mut Vec<Event>, name: &'static str, value: Value) {
    events.push(Event::Action(name, value));
}
fn button(ui: &mut Ui, key: &str) -> bool {
    ui.button(label(strings::Key::run_time(key))).clicked()
}
fn field(ui: &mut Ui, key: &str, value: &mut String) -> bool {
    ui.label(label(strings::Key::run_time(key)));
    ui.text_edit_singleline(value).changed()
}
fn display_text(v: &Value) -> String {
    v.as_str()
        .map(str::to_owned)
        .or_else(|| v.get("en").and_then(Value::as_str).map(str::to_owned))
        .or_else(|| v.as_object()?.values().find_map(Value::as_str).map(str::to_owned))
        .unwrap_or_default()
}

impl EditorView {
    pub fn show(&mut self, ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
        let Some(editor) = shell.symbol_editor() else {
            return;
        };
        if self.generation != Some(editor.generation) {
            *self = Self {
                generation: Some(editor.generation),
                ..Default::default()
            };
        }
        if self.zoom == 0.0 {
            self.zoom = 8.0;
        }
        if self.directory.is_empty() {
            self.directory = editor.directory.clone();
        }
        let draft = &editor.draft;
        let t = theme::current(ui.ctx());
        ui.horizontal_wrapped(|ui| {
            ui.heading(label(strings::Key::run_time("symbol-title")));
            ui.label(label(strings::Key::run_time(if editor.dirty() {
                "symbol-dirty"
            } else {
                "symbol-saved"
            })));
            if ui
                .add_enabled(
                    editor.can_undo(),
                    egui::Button::new(label(strings::Key::run_time("symbol-undo"))),
                )
                .clicked()
            {
                action(events, "symbol.undo", json!({}));
            }
            if ui
                .add_enabled(
                    editor.can_redo(),
                    egui::Button::new(label(strings::Key::run_time("symbol-redo"))),
                )
                .clicked()
            {
                action(events, "symbol.redo", json!({}));
            }
            if button(ui, "symbol-save") {
                action(events, "symbol.save", json!({"directory":self.directory}));
            }
            if button(ui, "symbol-close") {
                action(events, "symbol.close", json!({"discard":self.discard}));
            }
            ui.checkbox(&mut self.discard, label(strings::Key::run_time("symbol-discard")));
        });
        ui.label(label(strings::Key::run_time(
            if draft.symbol.grid == symbols::units::Grid::QET_10PX {
                "symbol-qet-grid"
            } else {
                "symbol-native-grid"
            },
        )));
        ui.separator();
        let width = (ui.available_width() * 0.27).clamp(260.0, 430.0);
        let height = ui.available_height();
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(Vec2::new(width,height),egui::Layout::top_down(egui::Align::Min),|ui| {
                egui::ScrollArea::vertical().id_salt("symbol-properties-scroll").max_height(height).show(ui,|ui| {
                    ui.set_min_width(width - 20.0);
                    self.properties(ui,draft,events);
                    if editor.candidates.len() > 1 { ui.collapsing(label(strings::Key::run_time("symbol-candidates")), |ui| {
                        for (i,d) in editor.candidates.iter().enumerate() {
                            if ui.button(display_text(&serde_json::to_value(&d.symbol.name).unwrap_or_default())).clicked() {
                                action(events,"symbol.candidate",json!({"index":i,"discard":self.discard}));
                            }
                        }
                    }); }
                    ui.separator();
                    ui.collapsing(label(strings::Key::run_time("symbol-import")),|ui| {
                        field(ui,"symbol-file",&mut self.file);
                        if button(ui,"symbol-import") { action(events,"symbol.open",json!({"file":self.file,"key":draft.key,"discard":self.discard})); }
                        if button(ui,"symbol-export") { action(events,"symbol.export",json!({"file":self.file})); }
                    });
                    field(ui,"symbol-library-folder",&mut self.directory);
                    ui.collapsing(label(strings::Key::run_time("symbol-place")),|ui| {
                        ui.horizontal(|ui| {
                            ui.label(label(strings::Key::run_time("symbol-x-mm"))); ui.add(egui::DragValue::new(&mut self.place[0]).speed(4.0));
                            ui.label(label(strings::Key::run_time("symbol-y-mm"))); ui.add(egui::DragValue::new(&mut self.place[1]).speed(4.0));
                        });
                        if ui.add_enabled(shell.has_project(),egui::Button::new(label(strings::Key::run_time("symbol-place")))).clicked() { action(events,"symbol.place",json!({"x":self.place[0],"y":self.place[1]})); }
                        if ui.add_enabled(shell.has_project(),egui::Button::new(label(strings::Key::run_time("symbol-scratch")))).clicked() { action(events,"symbol.scratch",json!({})); }
                    });
                    ui.separator(); ui.strong(label(strings::Key::run_time("symbol-validation")));
                    let issues = draft.validate();
                    if issues.is_empty() { ui.label(label(strings::Key::run_time("symbol-valid"))); }
                    for issue in issues { ui.colored_label(ui.visuals().error_fg_color, strings::format(strings::Key::run_time(issue.code),&[&issue.detail])); }
                    if let Some(error) = &editor.error { ui.colored_label(ui.visuals().error_fg_color,error); }
                    if !draft.import_notes.is_empty() { ui.collapsing(label(strings::Key::run_time("symbol-import-notes")),|ui| {
                        for note in &draft.import_notes { ui.label(label(strings::Key::run_time(note))); }
                        for detail in &draft.import_details { ui.label(&detail.message); }
                        let mut reviewed=draft.import_reviewed;
                        if ui.checkbox(&mut reviewed,label(strings::Key::run_time("symbol-reviewed"))).changed() {
                            let mut d=draft.clone();d.import_reviewed=reviewed;action(events,"symbol.replace",json!({"document":d}));
                        }
                    }); }
                });
            });
            ui.separator();
            ui.vertical(|ui| {
                let tools = ["select","line","rectangle","circle","ellipse","arc","text","cp","slot","anchor","link","polyline","polygon"];
                ui.horizontal_wrapped(|ui| { for (i,name) in tools.iter().enumerate() {
                    if ui.selectable_label(self.tool == i,label(strings::Key::run_time(&format!("symbol-{name}")))).clicked() { self.tool = i; self.start = None; self.vertices.clear(); }
                }});
                if self.tool == 6 { field(ui,"symbol-text",&mut self.text); }
                if self.tool == 10 { field(ui,"symbol-name",&mut self.link_name); }
                if self.tool == 8 { slot_kind_control(ui,&mut self.slot_kind,&mut self.slot_pin); }
                if self.tool == 7 { ui.horizontal_wrapped(|ui| {
                    field(ui,"symbol-designation",&mut self.designation);
                    egui::ComboBox::from_id_salt("symbol-cp-direction").selected_text(label(strings::Key::run_time(["symbol-up","symbol-right","symbol-down","symbol-left"][self.direction]))).show_ui(ui,|ui| {
                        for (i,k) in ["symbol-up","symbol-right","symbol-down","symbol-left"].iter().enumerate() { ui.selectable_value(&mut self.direction,i,label(strings::Key::run_time(k))); }
                    });
                }); }
                ui.label(label(strings::Key::run_time("symbol-draw-hint")));
                if self.tool>=11 {ui.label(label(strings::Key::run_time("symbol-path-hint")));}
                let canvas_height = (ui.available_height() - 122.0).max(120.0);
                let (rect,response) = ui.allocate_exact_size(Vec2::new(ui.available_width(),canvas_height),Sense::click_and_drag());
                let painter = ui.painter_at(rect);
                painter.rect_filled(rect,t.radius_none,t.canvas_sheet.color32());
                if response.dragged_by(egui::PointerButton::Middle) { self.manual_view=true; self.pan += ui.input(|i|i.pointer.delta()); }
                if response.hovered() {
                    let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                    if scroll != 0.0 {self.manual_view=true;}
                    self.zoom = (self.zoom * (scroll * 0.0015).exp()).clamp(1.0,80.0);
                }
                if !self.manual_view && let Ok(g)=draft.preview(editor.variant) {
                    let b=g.bounds;let width=(b.max().x.0-b.min().x.0) as f32/1e6;let height=(b.max().y.0-b.min().y.0) as f32/1e6;
                    self.zoom=(rect.width()/(width+16.0)).min(rect.height()/(height+16.0)).clamp(0.01,8.0);
                    self.pan=-Vec2::new((b.min().x.0+b.max().x.0) as f32/2e6,(b.min().y.0+b.max().y.0) as f32/2e6)*self.zoom;
                }
                let origin = rect.center() + self.pan;
                let step = self.zoom * draft.symbol.grid.at(1).map_or(4.0,|n|n.0 as f32 / 1_000_000.0);
                if step >= 5.0 {
                    let x0 = ((rect.left()-origin.x)/step).floor() as i32;
                    let y0 = ((rect.top()-origin.y)/step).floor() as i32;
                    for x in x0..x0+((rect.width()/step).ceil() as i32)+1 { for y in y0..y0+((rect.height()/step).ceil() as i32)+1 {
                        painter.circle_filled(origin+Vec2::new(x as f32*step,y as f32*step),0.8,t.canvas_grid_dot.color32());
                    }}
                }
                painter.line_segment([origin-Vec2::new(8.0,0.0),origin+Vec2::new(8.0,0.0)],Stroke::new(t.border_width,t.canvas_grid_dot.color32()));
                painter.line_segment([origin-Vec2::new(0.0,8.0),origin+Vec2::new(0.0,8.0)],Stroke::new(t.border_width,t.canvas_grid_dot.color32()));
                paint(&painter,draft,editor.variant,origin,self.zoom,true);
                for p in self.vertices.windows(2) {painter.line_segment([origin+Vec2::new(p[0][0] as f32,p[0][1] as f32)*self.zoom,origin+Vec2::new(p[1][0] as f32,p[1][1] as f32)*self.zoom],Stroke::new(t.border_width,t.canvas_selection.color32()));}
                if editor.variant != 0 { painter.text(rect.left_top()+Vec2::splat(8.0),egui::Align2::LEFT_TOP,label(strings::Key::run_time("symbol-edit-a")),theme::font(&t.type_body),t.canvas_selection.color32()); }
                if editor.variant == 0 && self.tool == 0 {
                    if response.clicked() && let Some(pointer)=response.interact_pointer_pos() {
                        self.selected=pick_graphic(draft,(pointer-origin)/self.zoom,5.0/self.zoom);
                    }
                    if let Some(index)=self.selected && let Some(bounds)=graphic_bounds(draft,index) {
                        let lo=origin+Vec2::new(bounds[0],bounds[1])*self.zoom;
                        let hi=origin+Vec2::new(bounds[2],bounds[3])*self.zoom;
                        painter.rect_stroke(egui::Rect::from_two_pos(lo,hi).expand(3.0),t.radius_none,Stroke::new(t.border_width,t.canvas_selection.color32()),egui::StrokeKind::Outside);
                        if response.drag_stopped_by(egui::PointerButton::Primary) {
                            let delta=response.drag_delta()/self.zoom;
                            let dx=(f64::from(delta.x)*2.0).round() as i64*500_000;
                            let dy=(f64::from(delta.y)*2.0).round() as i64*500_000;
                            if dx!=0||dy!=0 { let mut d=serde_json::to_value(draft).unwrap_or_default();translate_graphic(&mut d["symbol"]["graphics"][index],dx,dy);action(events,"symbol.replace",json!({"document":d})); }
                        }
                    }
                }
                if let Some(start) = self.start {
                    let at = origin+Vec2::new(start[0] as f32*self.zoom,start[1] as f32*self.zoom);
                    painter.circle_stroke(at,4.0,Stroke::new(t.focus_width,t.canvas_selection.color32()));
                    if let Some(pointer) = response.hover_pos() { painter.line_segment([at,pointer],Stroke::new(t.border_width,t.canvas_selection.color32())); }
                }
                if editor.variant == 0 && response.clicked() && let Some(pointer) = response.interact_pointer_pos() {
                    let at = (pointer-origin)/self.zoom;
                    let snap = if self.tool == 7 { draft.symbol.grid.at(1).map_or(4.0,|n|n.0 as f64/1_000_000.0) } else if matches!(self.tool,6|8) {1.0} else {0.5};
                    let mut p = [(f64::from(at.x)/snap).round()*snap,(f64::from(at.y)/snap).round()*snap];
                    if self.tool == 7 {
                        for v in &mut p { let grid=draft.symbol.grid; *v=grid.at(grid.snap(symbols::units::Nm((*v*1_000_000.0).round() as i64))).map_or(*v,|n|n.0 as f64/1_000_000.0); }
                        action(events,"symbol.cp",json!({"x":p[0],"y":p[1],"designation":self.designation,"direction":(["up","right","down","left"][self.direction])}));
                    } else if self.tool == 8 { action(events,"symbol.slot",json!({"x":p[0],"y":p[1],"kind":slot_kind(self.slot_kind,self.slot_pin)})); }
                    else if self.tool == 10 { action(events,"symbol.link",json!({"x":p[0],"y":p[1],"name":self.link_name})); }
                    else if self.tool >= 11 {
                        if self.vertices.last()!=Some(&p){self.vertices.push(p);}
                        if response.double_clicked() && self.vertices.len()>=if self.tool==12{3}else{2} {
                            action(events,"symbol.path",json!({"points":self.vertices,"closed":self.tool==12}));self.vertices.clear();
                        }
                    }
                    else if matches!(self.tool,6|9) { action(events,"symbol.draw",json!({"tool":tools[self.tool],"x":p[0],"y":p[1],"text":self.text})); }
                    else if self.tool > 0 {
                        if let Some(start)=self.start.take() { action(events,"symbol.draw",json!({"tool":tools[self.tool],"x":start[0],"y":start[1],"x2":p[0],"y2":p[1]})); } else { self.start=Some(p); }
                    }
                }
                if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE,egui::Key::Escape)) { self.start=None;self.vertices.clear(); }
                ui.horizontal(|ui| {
                    ui.label(label(strings::Key::run_time("symbol-variants")));
                    if button(ui,"symbol-fit") { self.manual_view=false; }
                });
                ui.horizontal(|ui| {
                    let w = ((ui.available_width()-7.0*ui.spacing().item_spacing.x)/8.0).max(30.0);
                    for variant in 0..8u8 {
                        let (r,response)=ui.allocate_exact_size(Vec2::new(w,78.0),Sense::click());
                        let painter=ui.painter_at(r);
                        painter.rect_filled(r,t.radius_mark,if variant==editor.variant {t.canvas_selection.alpha(24).over(t.canvas_sheet).color32()} else {t.canvas_sheet.color32()});
                        painter.text(r.left_top()+Vec2::new(4.0,2.0),egui::Align2::LEFT_TOP,char::from(b'A'+variant),theme::font(&t.type_small),t.canvas_sheet_line.color32());
                        if let Ok(g)=draft.preview(variant) {
                            let b=g.bounds; let width=(b.max().x.0-b.min().x.0) as f32/1_000_000.0;
                            let height=(b.max().y.0-b.min().y.0) as f32/1_000_000.0;
                            let scale=((w-12.0)/width.max(8.0)).min(54.0/height.max(8.0));
                            let centre=Vec2::new((b.max().x.0+b.min().x.0) as f32/2_000_000.0,(b.max().y.0+b.min().y.0) as f32/2_000_000.0);
                            paint(&painter,draft,variant,r.center()-centre*scale,scale,false);
                        }
                        if response.clicked() { action(events,"symbol.variant",json!({"variant":variant})); }
                    }
                });
            });
        });
    }

    fn properties(&mut self, ui: &mut Ui, draft: &SymbolDraft, events: &mut Vec<Event>) {
        let Ok(mut doc) = serde_json::to_value(draft) else {
            return;
        };
        let mut changed = false;
        let mut name = display_text(&doc["symbol"]["name"]);
        if field(ui, "symbol-name", &mut name) {
            doc["symbol"]["name"]["en"] = json!(name);
            changed = true;
        }
        for (key, path) in [("symbol-key", "key"), ("symbol-version", "version")] {
            let mut value = doc[path].as_str().unwrap_or_default().to_owned();
            if field(ui, key, &mut value) {
                doc[path] = json!(value);
                changed = true;
            }
        }
        ui.collapsing(label(strings::Key::run_time("symbol-properties")), |ui| {
            for key in ["licence", "sources"] {
                let mut value = if key == "sources" {
                    doc["symbol"]["meta"][key][0]
                        .as_str()
                        .unwrap_or_default()
                        .to_owned()
                } else {
                    doc["symbol"]["meta"][key].as_str().unwrap_or_default().to_owned()
                };
                if field(
                    ui,
                    if key == "sources" {
                        "symbol-source"
                    } else {
                        "symbol-licence"
                    },
                    &mut value,
                ) {
                    if key == "sources" {
                        if doc["symbol"]["meta"][key].as_array().is_none_or(Vec::is_empty) {
                            doc["symbol"]["meta"][key] = json!([value]);
                        } else {
                            doc["symbol"]["meta"][key][0] = json!(value);
                        }
                    } else {
                        doc["symbol"]["meta"][key] = json!(value);
                    }
                    changed = true;
                }
            }
            let mut centre = doc["symbol"]["anchor"] == "graphic_centre";
            if ui
                .checkbox(&mut centre, label(strings::Key::run_time("symbol-centre")))
                .changed()
            {
                doc["symbol"]["anchor"] = json!(if centre { "graphic_centre" } else { "origin" });
                changed = true;
            }
        });
        ui.collapsing(label(strings::Key::run_time("symbol-graphics")), |ui| {
            for (i, g) in draft.symbol.graphics.iter().enumerate() {
                let value = serde_json::to_value(g).unwrap_or_default();
                let kind = value
                    .as_object()
                    .and_then(|m| m.keys().next())
                    .map(String::as_str)
                    .unwrap_or("");
                let title = strings::format(
                    strings::Key::run_time("symbol-graphic-item"),
                    &[
                        label(strings::Key::run_time(&format!(
                            "symbol-{}",
                            if kind == "rect" { "rectangle" } else { kind }
                        ))),
                        &(i + 1).to_string(),
                    ],
                );
                if ui.selectable_label(self.selected == Some(i), title).clicked() {
                    self.selected = Some(i);
                }
            }
            if let Some(i) = self.selected
                && let Some(value) = doc["symbol"]["graphics"].get_mut(i)
            {
                changed |= numeric_fields(ui, value, "", false);
                if let Some(text) = value.get_mut("text") {
                    let mut s = display_text(&text["text"]);
                    if field(ui, "symbol-text", &mut s) {
                        text["text"]["en"] = json!(s);
                        changed = true;
                    }
                    changed |= rotation_control(ui, text);
                }
                if let Some(arc) = value.get_mut("arc") {
                    for key in ["start", "sweep"] {
                        let mut degrees = arc[key].as_f64().unwrap_or(0.0) / 1000.0;
                        ui.horizontal(|ui| {
                            ui.label(label(strings::Key::run_time(if key == "start" {
                                "symbol-arc-start"
                            } else {
                                "symbol-arc-sweep"
                            })));
                            if ui
                                .add(egui::DragValue::new(&mut degrees).range(-360.0..=360.0))
                                .changed()
                            {
                                arc[key] = json!((degrees * 1000.0).round() as i32);
                                changed = true;
                            }
                        });
                    }
                }
                if button(ui, "symbol-delete") {
                    if let Some(a) = doc["symbol"]["graphics"].as_array_mut() {
                        a.remove(i);
                    }
                    self.selected = None;
                    changed = true;
                }
            }
        });
        ui.collapsing(label(strings::Key::run_time("symbol-pins")), |ui| {
            let mut remove = None;
            if let Some(cps) = doc["symbol"]["cps"].as_array_mut() {
                for (i, cp) in cps.iter_mut().enumerate() {
                    ui.push_id(i, |ui| {
                        let name = draft
                            .function
                            .cps
                            .iter()
                            .find(|p| Some(u64::from(p.idx)) == cp["fn_cp"].as_u64())
                            .map(|p| p.designation.to_string())
                            .unwrap_or_else(|| (i + 1).to_string());
                        ui.strong(name);
                        changed |= numeric_fields(ui, &mut cp["pos"], "", true);
                        let mut mapped = cp["fn_cp"].as_u64().unwrap_or_default();
                        ui.horizontal(|ui| {
                            ui.label(label(strings::Key::run_time("symbol-function")));
                            if ui
                                .add(egui::DragValue::new(&mut mapped).range(0..=65535))
                                .changed()
                            {
                                cp["fn_cp"] = json!(mapped);
                                changed = true;
                            }
                        });
                        let current_direction = cp["dir"].clone();
                        egui::ComboBox::from_id_salt(("cp-direction", i))
                            .selected_text(label(strings::Key::run_time(&format!(
                                "symbol-{}",
                                match cp["dir"].as_str() {
                                    Some(d @ ("up" | "right" | "down" | "left")) => d,
                                    _ => "direction",
                                }
                            ))))
                            .show_ui(ui, |ui| {
                                for direction in ["up", "right", "down", "left"] {
                                    if ui
                                        .selectable_label(
                                            current_direction == direction,
                                            label(strings::Key::run_time(&format!("symbol-{direction}"))),
                                        )
                                        .clicked()
                                    {
                                        cp["dir"] = json!(direction);
                                        changed = true;
                                    }
                                }
                            });
                        if button(ui, "symbol-delete") {
                            remove = Some(i);
                        }
                    });
                }
            }
            if let Some(i) = remove
                && let Some(a) = doc["symbol"]["cps"].as_array_mut()
            {
                a.remove(i);
                changed = true;
            }
        });
        ui.collapsing(label(strings::Key::run_time("symbol-function")), |ui| {
            field(ui, "symbol-fndef-key", &mut self.function_key);
            if button(ui, "symbol-assign") {
                action(events, "symbol.function", json!({"key":self.function_key}));
            }
            ui.add_enabled_ui(draft.function_ref.is_none(), |ui| {
                if let Some(cps) = doc["function"]["cps"].as_array_mut() {
                    for (i, cp) in cps.iter_mut().enumerate() {
                        ui.push_id(("fn", i), |ui| {
                            let mut value = cp["designation"].as_str().unwrap_or_default().to_owned();
                            if field(ui, "symbol-designation", &mut value) {
                                cp["designation"] = json!(value);
                                changed = true;
                            }
                        });
                    }
                }
            });
        });
        ui.collapsing(label(strings::Key::run_time("symbol-slots")), |ui| {
            let mut remove = None;
            if let Some(slots) = doc["symbol"]["text_slots"].as_array_mut() {
                for (i, slot) in slots.iter_mut().enumerate() {
                    ui.push_id(("slot", i), |ui| {
                        let kind = slot["kind"].as_str().unwrap_or("cp_designation");
                        let key = match kind {
                            "tag" => "symbol-tag",
                            "function_text" => "symbol-function-text",
                            "technical_data" => "symbol-tech-data",
                            "cross_reference" => "symbol-xref",
                            _ => "symbol-cp-designation",
                        };
                        ui.label(label(strings::Key::run_time(key)));
                        changed |= numeric_fields(ui, &mut slot["pos"], "", true);
                        changed |= rotation_control(ui, slot);
                        let mut visible = slot["visible"].as_bool().unwrap_or(false);
                        if ui
                            .checkbox(&mut visible, label(strings::Key::run_time("symbol-visible")))
                            .changed()
                        {
                            slot["visible"] = json!(visible);
                            changed = true;
                        }
                        if button(ui, "symbol-delete") {
                            remove = Some(i);
                        }
                    });
                }
            }
            if let Some(i) = remove
                && let Some(a) = doc["symbol"]["text_slots"].as_array_mut()
            {
                a.remove(i);
                changed = true;
            }
        });
        ui.collapsing(label(strings::Key::run_time("symbol-link")), |ui| {
            let mut remove = None;
            if let Some(points) = doc["symbol"]["link_points"].as_array_mut() {
                for (i, p) in points.iter_mut().enumerate() {
                    ui.push_id(("link", i), |ui| {
                        let mut s = p["name"].as_str().unwrap_or_default().to_owned();
                        if field(ui, "symbol-name", &mut s) {
                            p["name"] = json!(s);
                            changed = true;
                        }
                        changed |= numeric_fields(ui, &mut p["pos"], "", true);
                        if button(ui, "symbol-delete") {
                            remove = Some(i);
                        }
                    });
                }
            }
            if let Some(i) = remove
                && let Some(a) = doc["symbol"]["link_points"].as_array_mut()
            {
                a.remove(i);
                changed = true;
            }
        });
        if changed {
            action(events, "symbol.replace", json!({"document":doc}));
        }
        ui.collapsing(label(strings::Key::run_time("symbol-advanced")), |ui| {
            if self.advanced.is_empty() {
                self.advanced = serde_json::to_string_pretty(draft).unwrap_or_default();
            }
            ui.add(
                egui::TextEdit::multiline(&mut self.advanced)
                    .code_editor()
                    .desired_rows(12)
                    .desired_width(ui.available_width()),
            );
            if button(ui, "symbol-refresh") {
                self.advanced = serde_json::to_string_pretty(draft).unwrap_or_default();
                self.json_error = false;
            }
            if button(ui, "symbol-apply") {
                match serde_json::from_str::<Value>(&self.advanced) {
                    Ok(value) => {
                        action(events, "symbol.replace", json!({"document":value}));
                        self.json_error = false;
                    }
                    Err(_) => self.json_error = true,
                }
            }
            if self.json_error {
                ui.colored_label(
                    ui.visuals().error_fg_color,
                    label(strings::Key::run_time("symbol-json-invalid")),
                );
            }
        });
    }
}

fn numeric_fields(ui: &mut Ui, value: &mut Value, name: &str, coordinate: bool) -> bool {
    let mut changed = false;
    match value {
        Value::Object(m) => {
            for (key, v) in m {
                if ["from", "to", "centre", "rect", "points", "pos", "r", "rx", "ry"].contains(&key.as_str())
                    || v.is_object()
                {
                    ui.push_id(key, |ui| {
                        changed |= numeric_fields(ui, v, key, true);
                    });
                }
            }
        }
        Value::Array(a) => {
            for (i, v) in a.iter_mut().enumerate() {
                ui.push_id((name, i), |ui| {
                    changed |= numeric_fields(ui, v, if i == 0 { "X" } else { "Y" }, coordinate);
                });
            }
        }
        Value::Number(n) if coordinate => {
            let mut mm = n.as_f64().unwrap_or_default() / 1_000_000.0;
            ui.horizontal(|ui| {
                ui.label(strings::format(strings::Key::run_time("symbol-mm"), &[name]));
                if ui
                    .add(
                        egui::DragValue::new(&mut mm)
                            .speed(0.5)
                            .range(-1_000_000.0..=1_000_000.0),
                    )
                    .changed()
                {
                    *value = json!((mm * 1_000_000.0).round() as i64);
                    changed = true;
                }
            });
        }
        _ => {}
    }
    changed
}

fn paint(painter: &egui::Painter, draft: &SymbolDraft, variant: u8, origin: Pos2, scale: f32, texts: bool) {
    let t = theme::current(painter.ctx());
    let Ok(g) = draft.preview(variant) else {
        return;
    };
    let at = |p: symbols::units::Pt2| {
        origin + Vec2::new(p.x.0 as f32 / 1_000_000.0, p.y.0 as f32 / 1_000_000.0) * scale
    };
    paint_graphics(painter, &g.graphics, variant, origin, scale);
    for cp in &g.cps {
        let p = at(cp.pos);
        painter.circle_stroke(
            p,
            if texts { 3.5 } else { 1.8 },
            Stroke::new(t.border_width, t.canvas_selection.color32()),
        );
        if texts
            && !draft
                .symbol
                .text_slots
                .iter()
                .any(|s| s.visible && s.kind == symbols::schema::SlotKind::CpDesignation(cp.idx))
        {
            let name = draft
                .function
                .cps
                .iter()
                .find(|c| c.idx == cp.fn_cp)
                .map(|c| c.designation.as_str())
                .unwrap_or("");
            painter.text(
                p + Vec2::new(5.0, -5.0),
                egui::Align2::LEFT_BOTTOM,
                name,
                theme::font(&t.type_small),
                t.canvas_selection.color32(),
            );
        }
    }
    {
        for text in &g.texts {
            if !text.visible {
                continue;
            }
            let content = match &text.source {
                symbols::TextSource::Static(t) => display_text(&serde_json::to_value(t).unwrap_or_default()),
                symbols::TextSource::Slot(symbols::schema::SlotKind::CpDesignation(idx)) => draft
                    .symbol
                    .cps
                    .iter()
                    .find(|c| c.idx == *idx)
                    .and_then(|c| draft.function.cps.iter().find(|f| f.idx == c.fn_cp))
                    .map(|f| f.designation.to_string())
                    .unwrap_or_default(),
                symbols::TextSource::Slot(symbols::schema::SlotKind::Tag) => "-U1".into(),
                symbols::TextSource::Slot(_) => label(strings::Key::run_time("symbol-text")).into(),
            };
            use symbols::schema::Align;
            let align = match text.align {
                Align::TopCentre => egui::Align2::CENTER_TOP,
                Align::TopRight => egui::Align2::RIGHT_TOP,
                Align::MiddleLeft => egui::Align2::LEFT_CENTER,
                Align::Centre => egui::Align2::CENTER_CENTER,
                Align::MiddleRight => egui::Align2::RIGHT_CENTER,
                Align::BottomLeft => egui::Align2::LEFT_BOTTOM,
                Align::BottomCentre => egui::Align2::CENTER_BOTTOM,
                Align::BottomRight => egui::Align2::RIGHT_BOTTOM,
                _ => egui::Align2::LEFT_TOP,
            };
            let galley = painter.layout_no_wrap(
                content,
                egui::FontId::proportional((2.0 * scale).clamp(3.0, 160.0)),
                t.canvas_sheet_line.color32(),
            );
            let rect = align.anchor_size(at(text.pos), galley.size());
            painter.add(
                egui::epaint::TextShape::new(rect.min, galley, t.canvas_sheet_line.color32())
                    .with_angle_and_anchor(-(text.angle.to_degrees_f64() as f32).to_radians(), align),
            );
        }
        if texts {
            use symbols::units::Orient2;
            let orientation = [
                Orient2::R0,
                Orient2::R90,
                Orient2::R180,
                Orient2::R270,
                Orient2::M0,
                Orient2::M90,
                Orient2::M180,
                Orient2::M270,
            ][usize::from(variant)];
            for point in &draft.symbol.link_points {
                let pos = at(orientation.apply(point.pos));
                painter.circle_stroke(pos, 4.0, Stroke::new(t.border_width, t.canvas_snap.color32()));
                painter.text(
                    pos + Vec2::new(5.0, 5.0),
                    egui::Align2::LEFT_TOP,
                    point.name.as_str(),
                    theme::font(&t.type_small),
                    t.canvas_snap.color32(),
                );
            }
            painter.circle_stroke(
                at(g.anchor),
                6.0,
                Stroke::new(t.border_width, t.canvas_snap.color32()),
            );
        }
    }
}

fn rotation_control(ui: &mut Ui, value: &mut Value) -> bool {
    let mut degrees = value["rot"].as_f64().unwrap_or(0.0) / 1000.0;
    ui.horizontal(|ui| {
        ui.label(label(strings::Key::run_time("symbol-rotation")));
        if ui
            .add(egui::DragValue::new(&mut degrees).range(-360.0..=360.0))
            .changed()
        {
            value["rot"] = json!((degrees * 1000.0).round() as i32);
            true
        } else {
            false
        }
    })
    .inner
}
fn slot_kind(index: usize, pin: u16) -> Value {
    match index {
        1 => json!("function_text"),
        2 => json!("tech_data"),
        3 => json!({"cp_designation":pin}),
        4 => json!("xref"),
        _ => json!("tag"),
    }
}
fn slot_kind_control(ui: &mut Ui, index: &mut usize, pin: &mut u16) {
    let keys = [
        "symbol-tag",
        "symbol-function-text",
        "symbol-tech-data",
        "symbol-cp-designation",
        "symbol-xref",
    ];
    egui::ComboBox::from_id_salt("symbol-new-slot")
        .selected_text(label(strings::Key::run_time(keys[*index])))
        .show_ui(ui, |ui| {
            for (i, k) in keys.iter().enumerate() {
                ui.selectable_value(index, i, label(strings::Key::run_time(k)));
            }
        });
    if *index == 3 {
        ui.add(egui::DragValue::new(pin));
    }
}
fn graphic_bounds(draft: &SymbolDraft, index: usize) -> Option<[f32; 4]> {
    let mut d = draft.clone();
    d.symbol.graphics = vec![d.symbol.graphics.get(index)?.clone()];
    d.symbol.cps = Default::default();
    d.symbol.text_slots.clear();
    d.symbol.link_points.clear();
    let b = d.preview(0).ok()?.bounds;
    Some([
        b.min().x.0 as f32 / 1e6,
        b.min().y.0 as f32 / 1e6,
        b.max().x.0 as f32 / 1e6,
        b.max().y.0 as f32 / 1e6,
    ])
}
fn pick_graphic(draft: &SymbolDraft, p: Vec2, slop: f32) -> Option<usize> {
    (0..draft.symbol.graphics.len()).rev().find(|&i| {
        graphic_bounds(draft, i).is_some_and(|b| {
            p.x >= b[0] - slop && p.y >= b[1] - slop && p.x <= b[2] + slop && p.y <= b[3] + slop
        })
    })
}
fn translate_graphic(v: &mut Value, dx: i64, dy: i64) {
    fn points(v: &mut Value, dx: i64, dy: i64) {
        if let Some(a) = v.as_array_mut() {
            if a.len() == 2 && a.iter().all(Value::is_i64) {
                a[0] = json!(a[0].as_i64().unwrap_or(0) + dx);
                a[1] = json!(a[1].as_i64().unwrap_or(0) + dy);
            } else {
                for v in a {
                    points(v, dx, dy);
                }
            }
        }
    }
    if let Some(map) = v.as_object_mut() {
        for (k, v) in map {
            if ["from", "to", "centre", "pos", "rect", "points"].contains(&k.as_str()) {
                points(v, dx, dy);
            } else {
                translate_graphic(v, dx, dy);
            }
        }
    }
}

/// The production CPU renderer handles fills, dashes and stroke widths. Cache one viewport per variant.
fn paint_graphics(
    painter: &egui::Painter,
    list: &drawing::DisplayList,
    variant: u8,
    origin: Pos2,
    scale: f32,
) {
    use std::hash::{Hash, Hasher};
    use symbols::units::{Nm, Pt2};
    let rect = painter.clip_rect();
    let shrink = (2048.0 / rect.width().max(rect.height())).min(1.0);
    let width = (rect.width() * shrink).ceil().max(1.0);
    let height = (rect.height() * shrink).ceil().max(1.0);
    let offset = (origin - rect.min) * shrink;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    list.hash(&mut h);
    for v in [width, height, offset.x, offset.y, scale] {
        v.to_bits().hash(&mut h);
    }
    let signature = h.finish();
    let id = egui::Id::unique(("symbol-raster", variant, rect.height() > 100.0));
    let cached = painter
        .ctx()
        .data(|d| d.get_temp::<(u64, egui::TextureHandle)>(id));
    let texture = if let Some((key, texture)) = cached.filter(|(key, _)| *key == signature) {
        let _ = key;
        texture
    } else {
        let placement = ew_render_cpu::PagePlacement {
            width: Nm((width * 100_000.0) as i64),
            height: Nm((height * 100_000.0) as i64),
            scale: (((scale * shrink * 100_000.0).round() as u64).max(1), 1_000_000),
            offset: Pt2::new(
                Nm((offset.x * 100_000.0) as i64),
                Nm((offset.y * 100_000.0) as i64),
            ),
            overlay: Default::default(),
        };
        let options = ew_render_cpu::RenderOptions {
            dpi: 254,
            background: drawing::Color::from_rgba8(0, 0, 0, 0),
        };
        let Ok((image, _)) = ew_render_cpu::render_placed(
            list,
            &placement,
            &options,
            &std::collections::BTreeMap::new(),
            None,
        ) else {
            return;
        };
        let texture = painter.ctx().load_texture(
            "symbol",
            egui::ColorImage::from_rgba_unmultiplied(
                [image.width as usize, image.height as usize],
                &image.pixels,
            ),
            egui::TextureOptions::LINEAR,
        );
        painter
            .ctx()
            .data_mut(|d| d.insert_temp(id, (signature, texture.clone())));
        texture
    };
    painter.image(
        texture.id(),
        rect,
        egui::Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        theme::NO_TINT,
    );
}
