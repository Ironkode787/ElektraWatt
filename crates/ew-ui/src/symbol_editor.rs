// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Symbol editor canvas and properties. All edits dispatch the shared headless actions.

use crate::{
    app::Event,
    components::{self, Kind},
    theme,
};
use egui::{Pos2, Sense, Stroke, Ui, Vec2};
use ew_viewmodel::command_bar::{ButtonKind, CommandButton, CommandSize};
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
    pending: Option<(&'static str, Value)>,
    panel: usize,
    zoom: f32,
    pan: Vec2,
    manual_view: bool,
    place: [f64; 2],
    draw_coordinates: [f64; 4],
    function_key: String,
    slot_kind: usize,
    slot_pin: u16,
    link_name: String,
    json_error: bool,
}

fn label(key: strings::Key<'_>) -> &'static str {
    strings::text(key)
}
fn control_name(name: &str, value: &str) -> String {
    strings::format(strings::Key::run_time("symbol-control-name"), &[name, value])
}
fn action(events: &mut Vec<Event>, name: &'static str, value: Value) {
    events.push(Event::Action(name, value));
}
fn button(ui: &mut Ui, key: &str) -> bool {
    components::button(ui, Kind::Secondary, label(strings::Key::run_time(key)), true, 0.0).clicked()
}
fn field(ui: &mut Ui, key: &str, value: &mut String) -> bool {
    let caption = ui.label(label(strings::Key::run_time(key)));
    let response = ui
        .add(egui::TextEdit::singleline(value).desired_width(ui.available_width()))
        .labelled_by(caption.id);
    response.widget_info(|| {
        let mut info = egui::WidgetInfo::labeled(
            egui::Role::TextInput,
            ui.is_enabled(),
            label(strings::Key::run_time(key)),
        );
        info.current_text_value = Some(value.clone());
        info
    });
    response.changed()
}
fn display_text(v: &Value) -> String {
    v.as_str()
        .map(str::to_owned)
        .or_else(|| v.get("en").and_then(Value::as_str).map(str::to_owned))
        .or_else(|| v.as_object()?.values().find_map(Value::as_str).map(str::to_owned))
        .unwrap_or_default()
}

pub(crate) fn toolbar_height(t: &theme::Tokens) -> f32 {
    2.0 * t.size_command_small + 3.0 * t.space_2
}

// Drawing action parameter, translated caption and the shared icon key.
const TOOLS: [(&str, &str, &str); 13] = [
    ("select", "symbol-select", "pointer"),
    ("line", "symbol-line", "ew-shape-line"),
    ("rectangle", "symbol-rectangle", "ew-shape-rectangle"),
    ("circle", "symbol-circle", "ew-shape-circle"),
    ("ellipse", "symbol-ellipse", "ew-shape-ellipse"),
    ("arc", "symbol-arc", "ew-shape-arc"),
    ("text", "symbol-text", "typography"),
    ("cp", "symbol-cp", "plug-connected"),
    ("slot", "symbol-slot", "text-size"),
    ("anchor", "symbol-anchor", "current-location"),
    ("link", "symbol-link", "link"),
    ("polyline", "symbol-polyline", "ew-shape-polyline"),
    ("polygon", "symbol-polygon", "ew-shape-polygon"),
];

fn ribbon_button(
    ui: &mut Ui,
    key: &'static str,
    icon: &'static str,
    checked: bool,
    enabled: bool,
    menu: bool,
) -> egui::Response {
    let name = label(strings::Key::run_time(key));
    crate::command_bar::inline_command(
        ui,
        &CommandButton {
            id: key,
            label: name.to_owned(),
            bar_label: name.to_owned(),
            icon,
            size: CommandSize::Small,
            shortcut: String::new(),
            enabled,
            reason: (!enabled).then(|| label(strings::Key::run_time("reason-no-project")).to_owned()),
            checked,
            topic: Some("symbol-editor"),
            kind: if menu {
                ButtonKind::Menu(Vec::new())
            } else {
                ButtonKind::Plain
            },
        },
    )
}

// MlText permits both neutral strings and language maps. Keep the form the author supplied.
fn edit_text(value: &mut Value, text: &str) {
    let Ok(mut typed) = serde_json::from_value::<symbols::MlText>(value.clone()) else {
        return;
    };
    match typed.view() {
        symbols::MlTextView::Neutral(_) => typed = symbols::MlText::neutral(text),
        symbols::MlTextView::Langs(map) => {
            let tag = map
                .keys()
                .find(|k| k.as_str() == "en")
                .or_else(|| map.keys().next())
                .cloned();
            if let Some(tag) = tag {
                typed.set_lang(tag, text);
            }
        }
    }
    *value = serde_json::to_value(typed).unwrap_or_default();
}

// Keep an in-progress empty translation in the widget, committing on Enter/focus loss.
fn text_field(ui: &mut Ui, key: &str, value: &mut Value) -> bool {
    let id = ui.make_persistent_id(key);
    let (source, mut buffer) = ui
        .data_mut(|d| d.get_temp::<(Value, String)>(id))
        .filter(|(source, _)| source == value)
        .unwrap_or_else(|| (value.clone(), display_text(value)));
    let caption = ui.label(label(strings::Key::run_time(key)));
    let response = ui
        .add(egui::TextEdit::singleline(&mut buffer).desired_width(ui.available_width()))
        .labelled_by(caption.id);
    response.widget_info(|| {
        let mut info = egui::WidgetInfo::labeled(
            egui::Role::TextInput,
            ui.is_enabled(),
            label(strings::Key::run_time(key)),
        );
        info.current_text_value = Some(buffer.clone());
        info
    });
    let commit = response.lost_focus() && buffer != display_text(value);
    ui.data_mut(|d| d.insert_temp(id, (source, buffer.clone())));
    if commit {
        edit_text(value, &buffer);
    }
    commit
}

fn function_pins(ui: &mut Ui, cps: &mut [Value]) -> bool {
    let mut changed = false;
    egui::Grid::new("symbol-function-pins")
        .spacing(Vec2::new(4.0, 6.0))
        .show(ui, |ui| {
            for key in [
                "symbol-pin-column",
                "symbol-role-column",
                "symbol-source-column",
                "symbol-optional-column",
            ] {
                ui.small(label(strings::Key::run_time(key)));
            }
            ui.end_row();
            for (i, cp) in cps.iter_mut().enumerate() {
                let mut designation = cp["designation"].as_str().unwrap_or_default().to_owned();
                let field = ui.add(
                    egui::TextEdit::singleline(&mut designation)
                        .id_salt(("pin", i))
                        .desired_width(38.0),
                );
                field.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::Role::TextInput,
                        ui.is_enabled(),
                        control_name(
                            label(strings::Key::run_time("symbol-designation")),
                            &(i + 1).to_string(),
                        ),
                    )
                });
                if field.changed() && !designation.is_empty() {
                    cp["designation"] = json!(designation);
                    changed = true;
                }
                let role = cp["potential_role"].as_str().unwrap_or("").to_owned();
                let shown = if role.is_empty() {
                    "—".to_owned()
                } else {
                    role.to_uppercase()
                        .replace("L_PLUS", "L+")
                        .replace("L_MINUS", "L-")
                };
                egui::ComboBox::from_id_salt(("role", i))
                    .width(65.0)
                    .selected_text(shown)
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(
                                role.is_empty(),
                                label(strings::Key::run_time("symbol-role-none")),
                            )
                            .clicked()
                        {
                            if let Some(map) = cp.as_object_mut() {
                                map.remove("potential_role");
                            }
                            changed = true;
                        }
                        for role in [
                            "l1",
                            "l2",
                            "l3",
                            "l",
                            "n",
                            "pe",
                            "pen",
                            "fe",
                            "l_plus",
                            "l_minus",
                            "m",
                            "signal",
                            "undefined",
                        ] {
                            changed |= ui
                                .selectable_value(
                                    &mut cp["potential_role"],
                                    json!(role),
                                    role.to_uppercase()
                                        .replace("L_PLUS", "L+")
                                        .replace("L_MINUS", "L-"),
                                )
                                .changed();
                        }
                    })
                    .response
                    .widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::Role::ComboBox,
                            ui.is_enabled(),
                            control_name(
                                label(strings::Key::run_time("symbol-potential-role")),
                                &designation,
                            ),
                        )
                    });
                for (member, key) in [
                    ("source_output", "symbol-source-output"),
                    ("optional", "symbol-optional-pin"),
                ] {
                    let mut on = cp[member].as_bool().unwrap_or(false);
                    let response = ui
                        .checkbox(&mut on, "")
                        .on_hover_text(label(strings::Key::run_time(key)));
                    response.widget_info(|| {
                        egui::WidgetInfo::selected(
                            egui::Role::CheckBox,
                            ui.is_enabled(),
                            on,
                            control_name(label(strings::Key::run_time(key)), &designation),
                        )
                    });
                    if response.changed() {
                        cp[member] = json!(on);
                        changed = true;
                    }
                }
                ui.end_row();
            }
        });
    changed
}

impl EditorView {
    fn sync(&mut self, shell: &ShellModel) {
        if let Some(editor) = shell.symbol_editor()
            && self.generation != Some(editor.generation)
        {
            *self = Self {
                generation: Some(editor.generation),
                directory: editor.directory.clone(),
                ..Default::default()
            };
        }
    }

    fn request(&mut self, shell: &ShellModel, events: &mut Vec<Event>, name: &'static str, value: Value) {
        if shell.symbol_editor().is_some_and(|e| e.dirty()) {
            self.pending = Some((name, value));
        } else {
            action(events, name, value);
        }
    }

    pub(crate) fn toolbar(&mut self, ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
        self.sync(shell);
        let Some(editor) = shell.symbol_editor() else {
            return;
        };
        let t = theme::current(ui.ctx());
        ui.spacing_mut().item_spacing = Vec2::splat(t.space_2);
        ui.horizontal(|ui| {
            for i in [0, 1, 2, 3] {
                self.tool_button(ui, i);
            }
            let more = ribbon_button(
                ui,
                "symbol-more-shapes",
                "category",
                matches!(self.tool, 4 | 5 | 11 | 12),
                true,
                true,
            );
            egui::Popup::menu(&more).show(|ui| {
                for i in [4, 5, 11, 12] {
                    if self.tool_button(ui, i) {
                        ui.close();
                    }
                }
            });
            ui.separator();
            for i in [6, 7, 8, 9, 10] {
                self.tool_button(ui, i);
            }
        });
        ui.horizontal(|ui| {
            if ribbon_button(ui, "symbol-save", "device-floppy", false, true, false).clicked() {
                if self.directory.is_empty() {
                    self.panel = 2;
                } else {
                    action(events, "symbol.save", json!({"directory":self.directory}));
                }
            }
            if ribbon_button(ui, "symbol-import", "file-import", false, true, false).clicked() {
                self.panel = 2;
            }
            if ribbon_button(ui, "symbol-scratch", "eye", false, shell.has_project(), false).clicked()
                && shell.has_project()
            {
                action(events, "symbol.scratch", json!({}));
            }
            if ribbon_button(ui, "symbol-close", "x", false, true, false).clicked() {
                self.request(shell, events, "symbol.close", json!({}));
            }
            ui.separator();
            ui.label(label(strings::Key::run_time(if editor.variant == 0 {
                "symbol-draw-hint"
            } else {
                "symbol-edit-a"
            })));
        });
    }

    fn tool_button(&mut self, ui: &mut Ui, index: usize) -> bool {
        let (_, caption, icon) = TOOLS[index];
        let clicked = ribbon_button(ui, caption, icon, self.tool == index, true, false).clicked();
        if clicked {
            self.tool = index;
            self.start = None;
            self.vertices.clear();
        }
        clicked
    }

    pub fn show(&mut self, ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
        self.sync(shell);
        let Some(editor) = shell.symbol_editor() else {
            return;
        };
        if let Some((name, mut value)) = self.pending.clone() {
            egui::Modal::new(egui::Id::unique("symbol-discard-confirm")).show(ui.ctx(), |ui| {
                ui.heading(label(strings::Key::run_time("symbol-dirty")));
                ui.label(label(strings::Key::run_time("symbol-unsaved")));
                ui.horizontal(|ui| {
                    if button(ui, "symbol-cancel") {
                        self.pending = None;
                    }
                    if components::button(
                        ui,
                        Kind::Danger,
                        label(strings::Key::run_time("symbol-discard")),
                        true,
                        0.0,
                    )
                    .clicked()
                    {
                        value["discard"] = json!(true);
                        action(events, name, value);
                        self.pending = None;
                    }
                });
            });
        }
        if self.zoom == 0.0 {
            self.zoom = 8.0;
        }
        if self.directory.is_empty() {
            self.directory = editor.directory.clone();
        }
        let draft = &editor.draft;
        let styles = shell.symbol_text_styles();
        let t = theme::current(ui.ctx());
        ui.horizontal(|ui| {
            ui.strong(display_text(
                &serde_json::to_value(&draft.symbol.name).unwrap_or_default(),
            ));
            ui.label(label(strings::Key::run_time(if editor.dirty() {
                "symbol-dirty"
            } else {
                "symbol-saved"
            })));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if button(ui, "symbol-fit") {
                    self.manual_view = false;
                }
                ui.label(label(strings::Key::run_time(
                    if draft.symbol.grid == symbols::units::Grid::QET_10PX {
                        "symbol-qet-grid"
                    } else {
                        "symbol-native-grid"
                    },
                )));
            });
        });
        ui.separator();
        let width =
            (t.size_panel_left + 0.35 * (ui.available_width() - 1280.0).max(0.0)).min(t.size_panel_left_max);
        let height = ui.available_height();
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(Vec2::new(width,height),egui::Layout::top_down(egui::Align::Min),|ui| {
                egui::ScrollArea::vertical().id_salt("symbol-properties-scroll").max_height(height).show(ui,|ui| {
                    ui.set_min_width(width - 20.0);
                    ui.horizontal(|ui| {
                        for (i, key) in ["symbol-drawing-tab", "symbol-data-tab", "symbol-library-tab"].iter().enumerate() {
                            ui.selectable_value(&mut self.panel, i, label(strings::Key::run_time(key)));
                        }
                    });
                    ui.separator();
                    if self.panel == 0 && self.tool != 0 { self.numeric_draw(ui, events); }
                    if !draft.import_notes.is_empty() && !draft.import_reviewed { ui.label(label(strings::Key::run_time("symbol-import-review"))); }
                    if !draft.import_notes.is_empty() { ui.collapsing(label(strings::Key::run_time("symbol-import-notes")),|ui| {
                        for note in &draft.import_notes { ui.label(label(strings::Key::run_time(note))); }
                        for detail in &draft.import_details { ui.label(&detail.message); }
                        let mut reviewed=draft.import_reviewed;
                        if ui.checkbox(&mut reviewed,label(strings::Key::run_time("symbol-reviewed"))).changed() {
                            let mut d=draft.clone();d.import_reviewed=reviewed;action(events,"symbol.replace",json!({"document":d}));
                        }
                    }); }
                    ui.push_id(editor.generation, |ui| self.properties(ui,draft,events));
                    if self.panel == 2 {
                    if editor.candidates.len() > 1 { ui.collapsing(label(strings::Key::run_time("symbol-candidates")), |ui| {
                        for (i,d) in editor.candidates.iter().enumerate() {
                            if ui.button(display_text(&serde_json::to_value(&d.symbol.name).unwrap_or_default())).clicked() {
                                self.request(shell, events,"symbol.candidate",json!({"index":i}));
                            }
                        }
                    }); }
                    ui.separator();
                    ui.collapsing(label(strings::Key::run_time("symbol-import")),|ui| {
                        field(ui,"symbol-file",&mut self.file);
                        if button(ui,"symbol-import") { self.request(shell, events,"symbol.open",json!({"file":self.file,"key":draft.key})); }
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
                    }
                    ui.separator(); ui.strong(label(strings::Key::run_time("symbol-validation")));
                    let issues = draft.validate();
                    if issues.is_empty() { ui.label(label(strings::Key::run_time("symbol-valid"))); }
                    for issue in issues { ui.colored_label(ui.visuals().error_fg_color, strings::format(strings::Key::run_time(issue.code),&[&issue.detail])); }
                    if let Some(error) = &editor.error { ui.colored_label(ui.visuals().error_fg_color,error); }

                });
            });
            ui.separator();
            ui.vertical(|ui| {
                let tools = TOOLS;
                if self.tool == 6 { field(ui,"symbol-text",&mut self.text); }
                if self.tool == 10 { field(ui,"symbol-name",&mut self.link_name); }
                if self.tool == 8 { slot_kind_control(ui,&mut self.slot_kind,&mut self.slot_pin); }
                if self.tool == 7 { ui.horizontal_wrapped(|ui| {
                    field(ui,"symbol-designation",&mut self.designation);
                    egui::ComboBox::from_id_salt("symbol-cp-direction").selected_text(label(strings::Key::run_time(["symbol-up","symbol-right","symbol-down","symbol-left"][self.direction]))).show_ui(ui,|ui| {
                        for (i,k) in ["symbol-up","symbol-right","symbol-down","symbol-left"].iter().enumerate() { ui.selectable_value(&mut self.direction,i,label(strings::Key::run_time(k))); }
                    });
                }); }
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
                paint(&painter,draft,editor.variant,origin,self.zoom,true,&styles);
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
                });
                ui.horizontal(|ui| {
                    let w = ((ui.available_width()-7.0*ui.spacing().item_spacing.x)/8.0).max(30.0);
                    for variant in 0..8u8 {
                        let (r,response)=ui.allocate_exact_size(Vec2::new(w,78.0),Sense::click());
                        response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, variant == editor.variant, control_name(label(strings::Key::run_time("symbol-variants")), &char::from(b'A'+variant).to_string())));
                        let painter=ui.painter_at(r);
                        painter.rect_filled(r,t.radius_mark,if variant==editor.variant {t.canvas_selection.alpha(24).over(t.canvas_sheet).color32()} else {t.canvas_sheet.color32()});
                        painter.text(r.left_top()+Vec2::new(4.0,2.0),egui::Align2::LEFT_TOP,char::from(b'A'+variant),theme::font(&t.type_small),t.canvas_sheet_line.color32());
                        if let Ok(g)=draft.preview(variant) {
                            let b=g.bounds; let width=(b.max().x.0-b.min().x.0) as f32/1_000_000.0;
                            let height=(b.max().y.0-b.min().y.0) as f32/1_000_000.0;
                            let scale=((w-12.0)/width.max(8.0)).min(54.0/height.max(8.0));
                            let centre=Vec2::new((b.max().x.0+b.min().x.0) as f32/2_000_000.0,(b.max().y.0+b.min().y.0) as f32/2_000_000.0);
                            paint(&painter,draft,variant,r.center()-centre*scale,scale,false,&styles);
                        }
                        if response.clicked() { action(events,"symbol.variant",json!({"variant":variant})); }
                    }
                });
            });
        });
    }

    fn numeric_draw(&mut self, ui: &mut Ui, events: &mut Vec<Event>) {
        egui::CollapsingHeader::new(label(strings::Key::run_time("symbol-numeric-draw"))).default_open(true).show(ui, |ui| {
            for (i, key) in ["symbol-x-mm", "symbol-y-mm", "symbol-end-x", "symbol-end-y"].iter().enumerate() {
                if i < 2 || matches!(self.tool, 1..=5) {
                    ui.horizontal(|ui| {
                        let caption = ui.label(label(strings::Key::run_time(key)));
                        let response = ui.add(egui::DragValue::new(&mut self.draw_coordinates[i]).speed(0.5)).labelled_by(caption.id);
                        response.widget_info(|| {
                            let mut info = egui::WidgetInfo::drag_value(ui.is_enabled(), self.draw_coordinates[i]);
                            info.label = Some(label(strings::Key::run_time(key)).to_owned());
                            info
                        });
                    });
                }
            }
            if button(ui, "symbol-add") {
                let [x, y, x2, y2] = self.draw_coordinates;
                match self.tool {
                    7 => action(events, "symbol.cp", json!({"x":x,"y":y,"designation":self.designation,"direction":(["up","right","down","left"][self.direction])})),
                    8 => action(events, "symbol.slot", json!({"x":x,"y":y,"kind":slot_kind(self.slot_kind,self.slot_pin)})),
                    10 => action(events, "symbol.link", json!({"x":x,"y":y,"name":self.link_name})),
                    11 | 12 => self.vertices.push([x,y]),
                    _ => action(events, "symbol.draw", json!({"tool":TOOLS[self.tool].0,"x":x,"y":y,"x2":x2,"y2":y2,"text":self.text})),
                }
            }
            if self.tool >= 11 && self.vertices.len() >= if self.tool == 12 { 3 } else { 2 } && button(ui, "symbol-finish-path") {
                action(events, "symbol.path", json!({"points":self.vertices,"closed":self.tool == 12})); self.vertices.clear();
            }
        });
    }

    fn properties(&mut self, ui: &mut Ui, draft: &SymbolDraft, events: &mut Vec<Event>) {
        let Ok(mut doc) = serde_json::to_value(draft) else {
            return;
        };
        let mut changed = false;
        changed |= text_field(ui, "symbol-name", &mut doc["symbol"]["name"]);
        if self.panel == 2 {
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
        }
        if self.panel == 0 {
            egui::CollapsingHeader::new(label(strings::Key::run_time("symbol-graphics")))
                .default_open(true)
                .show(ui, |ui| {
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
                            changed |= ui
                                .push_id(("graphic-text", i), |ui| {
                                    text_field(ui, "symbol-text", &mut text["text"])
                                })
                                .inner;
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
        }
        if self.panel == 1 {
            ui.label(label(strings::Key::run_time("symbol-data-help")));
            egui::CollapsingHeader::new(label(strings::Key::run_time("symbol-function")))
                .default_open(true)
                .show(ui, |ui| {
                    ui.collapsing(label(strings::Key::run_time("symbol-assign")), |ui| {
                        field(ui, "symbol-fndef-key", &mut self.function_key);
                        if button(ui, "symbol-assign") {
                            action(events, "symbol.function", json!({"key":self.function_key}));
                        }
                    });
                    ui.add_enabled_ui(draft.function_ref.is_none(), |ui| {
                        ui.label(label(strings::Key::run_time("symbol-function-kind")));
                        let kind = doc["function"]["contact_kind"].as_str().unwrap_or("").to_owned();
                        egui::ComboBox::from_id_salt("symbol-function-kind")
                            .selected_text(label(strings::Key::run_time(&format!("symbol-kind-{kind}"))))
                            .show_ui(ui, |ui| {
                                for kind in [
                                    "black_box",
                                    "no",
                                    "nc",
                                    "co",
                                    "coil",
                                    "pole",
                                    "terminal",
                                    "plc_io",
                                    "bus_port",
                                    "valve",
                                ] {
                                    if ui
                                        .selectable_value(
                                            &mut doc["function"]["contact_kind"],
                                            json!(kind),
                                            label(strings::Key::run_time(&format!("symbol-kind-{kind}"))),
                                        )
                                        .changed()
                                    {
                                        changed = true;
                                    }
                                }
                            });
                        if let Some(cps) = doc["function"]["cps"].as_array_mut() {
                            changed |= function_pins(ui, cps);
                        }
                    });
                });
            egui::CollapsingHeader::new(label(strings::Key::run_time("symbol-pins")))
                .default_open(false)
                .show(ui, |ui| {
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
                                        cp["dir"]
                                            .as_str()
                                            .filter(|d| matches!(*d, "up" | "right" | "down" | "left"))
                                            .unwrap_or("direction")
                                    ))))
                                    .show_ui(ui, |ui| {
                                        for direction in ["up", "right", "down", "left"] {
                                            if ui
                                                .selectable_label(
                                                    current_direction == direction,
                                                    label(strings::Key::run_time(&format!(
                                                        "symbol-{direction}"
                                                    ))),
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
        }
        if self.panel == 0 {
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
        }
        if changed {
            action(events, "symbol.replace", json!({"document":doc}));
        }
        if self.panel == 2 {
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

fn paint(
    painter: &egui::Painter,
    draft: &SymbolDraft,
    variant: u8,
    origin: Pos2,
    scale: f32,
    texts: bool,
    styles: &symbols::TextStyleTable,
) {
    let t = theme::current(painter.ctx());
    let Ok(g) = draft.preview(variant) else {
        return;
    };
    let at = |p: symbols::units::Pt2| {
        origin + Vec2::new(p.x.0 as f32 / 1_000_000.0, p.y.0 as f32 / 1_000_000.0) * scale
    };
    let list = symbols::preview_display_list(draft, &g, styles, label(strings::Key::run_time("symbol-text")));
    paint_graphics(painter, &list, variant, origin, scale);
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
            &ew_viewmodel::ew_api::output::font_bytes()
                .iter()
                .map(|(key, bytes)| {
                    (
                        *key,
                        ew_render_cpu::FontBytes {
                            data: bytes.data.clone(),
                            index: bytes.index,
                        },
                    )
                })
                .collect::<std::collections::BTreeMap<_, _>>(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_legal_neutral_and_multilingual_names_without_losing_other_languages() {
        let mut neutral = json!("24 V");
        edit_text(&mut neutral, "48 V");
        assert_eq!(neutral, json!("48 V"));
        let mut translated = json!({"en":"Power block", "nl":"Voedingsblok"});
        edit_text(&mut translated, "Power supply");
        assert_eq!(translated, json!({"en":"Power supply", "nl":"Voedingsblok"}));
        edit_text(&mut translated, "");
        assert_eq!(translated, json!({"nl":"Voedingsblok"}));
        let mut dutch = json!({"nl":"Voeding"});
        edit_text(&mut dutch, "Motor");
        assert_eq!(dutch, json!({"nl":"Motor"}));
    }
}
