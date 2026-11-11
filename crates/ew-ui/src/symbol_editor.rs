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
    /// The editor's library folder last seen, so a picked or saved folder shows in the field.
    seen_directory: String,
    tool: usize,
    /// The model's tool last seen, so a key that chooses a tool shows here.
    seen_tool: &'static str,
    start: Option<[f64; 2]>,
    /// The spot a Text tool click marked while the Text field was empty: the text typed is created there on Enter
    /// (F2).
    text_mark: Option<[f64; 2]>,
    vertices: Vec<[f64; 2]>,
    selected: Option<usize>,
    move_start: Option<Pos2>,
    advanced: String,
    text: String,
    designation: String,
    direction: usize,
    panel: usize,
    zoom: f32,
    pan: Vec2,
    manual_view: bool,
    /// The editor's size the view was last fitted at: the view fits again only at another size or on Fit,
    /// never because the refusal banner shortens the canvas.
    fitted: Option<Vec2>,
    place: [f64; 2],
    draw_coordinates: [f64; 4],
    arc_angles: [i32; 2],
    function_key: String,
    source: String,
    fork_key: String,
    source_filter: String,
    slot_kind: usize,
    slot_pin: u16,
    link_name: String,
    json_error: bool,
}

#[derive(Clone)]
struct FunctionPinDrag(u16);

fn label(key: strings::Key<'_>) -> &'static str {
    strings::text(key)
}
fn control_name(name: &str, value: &str) -> String {
    strings::format(strings::Key::run_time("symbol-control-name"), &[name, value])
}
fn function_choice_name(f: &ew_viewmodel::shell::symbol_editor::SymbolFunctionChoice) -> String {
    let kind = serde_json::to_value(&f.kind).unwrap_or_default();
    strings::format(
        strings::Key::run_time("symbol-function-choice"),
        &[
            &f.category,
            label(strings::Key::run_time(&format!(
                "symbol-kind-{}",
                kind.as_str().unwrap_or("unknown")
            ))),
            &f.pins.join(", "),
            &f.library,
        ],
    )
}
fn source_choice_name(s: &ew_viewmodel::shell::symbol_editor::SymbolSource) -> String {
    strings::format(
        strings::Key::run_time(if s.grid == symbols::units::Grid::QET_10PX {
            "symbol-source-qet"
        } else {
            "symbol-source-native"
        }),
        &[
            &display_text(&serde_json::to_value(&s.name).unwrap_or_default()),
            s.library
                .as_deref()
                .unwrap_or_else(|| label(strings::Key::run_time("symbol-project-copy"))),
        ],
    )
}
fn function_pin_name(draft: &SymbolDraft, index: u16) -> String {
    let Some(pin) = draft.function.cps.get(&index) else {
        return "—".to_owned();
    };
    if draft
        .function
        .cps
        .iter()
        .filter(|p| p.designation == pin.designation)
        .count()
        > 1
    {
        let ordinal = draft
            .function
            .cps
            .iter()
            .position(|p| p.idx == index)
            .unwrap_or_default()
            + 1;
        strings::format(
            strings::Key::run_time("symbol-pin-ordinal"),
            &[pin.designation.as_str(), &ordinal.to_string()],
        )
    } else {
        pin.designation.to_string()
    }
}
fn action(events: &mut Vec<Event>, name: &'static str, value: Value) {
    events.push(Event::Action(name, value));
}
/// An action that drops the draft: the model asks Save symbol, Discard or Cancel first while it has unsaved changes,
/// in the house dialog.
fn ask(events: &mut Vec<Event>, name: &'static str, value: Value) {
    events.push(Event::SymbolAsk(name, value));
}
// Several controls can commit on one focus change. Merge their changes against the same frame's draft.
fn replace_action(events: &mut Vec<Event>, draft: &SymbolDraft, document: Value) {
    fn merge(base: &Value, changed: &Value, target: &mut Value) {
        if base == changed {
            return;
        }
        if let (Some(base), Some(changed), Some(target)) =
            (base.as_object(), changed.as_object(), target.as_object_mut())
        {
            for key in base.keys().filter(|key| !changed.contains_key(*key)) {
                target.remove(key);
            }
            for (key, value) in changed {
                merge(
                    base.get(key).unwrap_or(&Value::Null),
                    value,
                    target.entry(key.clone()).or_insert(Value::Null),
                );
            }
        } else {
            *target = changed.clone();
        }
    }
    if let Some(Event::Action(_, params)) = events
        .iter_mut()
        .find(|event| matches!(event, Event::Action("symbol.replace", _)))
    {
        merge(
            &serde_json::to_value(draft).unwrap_or_default(),
            &document,
            &mut params["document"],
        );
    } else {
        // Commit a field losing focus before the ribbon's Save/Test action.
        events.insert(0, Event::Action("symbol.replace", json!({"document":document})));
    }
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
/// [`field`] for a text that may hold a line break, which Shift+Enter starts; Enter ends the entry.
/// Returns the field's input and whether Enter ended the entry.
fn lines_field(ui: &mut Ui, key: &str, value: &mut String) -> (egui::Id, bool) {
    let caption = ui.label(label(strings::Key::run_time(key)));
    let input = ui.make_persistent_id(("lines", key));
    let enter = enter_pressed(ui, input);
    let response = ui
        .add(
            egui::TextEdit::multiline(value)
                .id(input)
                .desired_rows(1)
                .return_key(new_line_key())
                .desired_width(ui.available_width()),
        )
        .labelled_by(caption.id);
    if enter {
        response.surrender_focus();
    }
    let name = label(strings::Key::run_time(key));
    ui.ctx()
        .accesskit_node_builder(response.id, |n| n.set_label(name));
    (input, enter)
}
/// A text of the document that commits on Enter or focus loss, never per keystroke (M25-p3#4): the typing stays in
/// the field until then. `allow_empty` false keeps the previous text when the field is left empty, so a designation
/// can be cleared and retyped.
fn committed_text(
    ui: &mut Ui,
    salt: impl std::hash::Hash + std::fmt::Debug,
    name: &str,
    caption: Option<&str>,
    value: &mut String,
    width: f32,
    allow_empty: bool,
) -> bool {
    let id = ui.make_persistent_id(salt);
    let (source, mut buffer) = ui
        .data_mut(|d| d.get_temp::<(String, String)>(id))
        .filter(|(source, _)| source == value)
        .unwrap_or_else(|| (value.clone(), value.clone()));
    let caption = caption.map(|c| ui.label(c));
    let mut response = ui.add(
        egui::TextEdit::singleline(&mut buffer)
            .id(id.with("input"))
            .desired_width(width),
    );
    if let Some(caption) = caption {
        response = response.labelled_by(caption.id);
    }
    response.widget_info(|| {
        let mut info = egui::WidgetInfo::labeled(egui::Role::TextInput, ui.is_enabled(), name);
        info.current_text_value = Some(buffer.clone());
        info
    });
    // M25-p5#2 (doc 16a §6.8): Escape cancels the entry; it is the field's, so the tool stays.
    let escape = response.lost_focus() && cancelled(ui);
    let commit =
        response.lost_focus() && !escape && buffer != *value && (allow_empty || !buffer.trim().is_empty());
    if response.lost_focus() && !commit {
        buffer.clone_from(value);
    }
    ui.data_mut(|d| d.insert_temp(id, (source, buffer.clone())));
    if commit {
        *value = buffer;
    }
    commit
}

/// Whether the field that lost the focus this frame lost it to Escape, which then is the field's and goes no further
/// (doc 16a §6.8: Escape cancels).
fn cancelled(ui: &Ui) -> bool {
    ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
}

/// A number of the document named `name`: typed text commits on Enter or focus loss, a drag at its end, never per
/// frame (M25-p3#4). Returns whether `value` changed.
fn number(
    ui: &mut Ui,
    value: &mut f64,
    speed: f64,
    range: std::ops::RangeInclusive<f64>,
    name: &str,
) -> bool {
    // M25-p5#3: a slot of its own beside the DragValue's id, whose precise drag value egui keeps as an `f64` too.
    let id = ui.next_auto_id().with("symbol-number");
    let mut v = ui.data(|d| d.get_temp::<f64>(id)).unwrap_or(*value);
    let response = ui.add(
        egui::DragValue::new(&mut v)
            .speed(speed)
            .range(range)
            .update_while_editing(false),
    );
    response.widget_info(|| {
        let mut info = egui::WidgetInfo::drag_value(ui.is_enabled(), v);
        info.label = Some(name.to_owned());
        info
    });
    if response.dragged() {
        ui.data_mut(|d| d.insert_temp(id, v));
        return false;
    }
    ui.data_mut(|d| d.remove::<f64>(id));
    let commit = (response.drag_stopped() || response.changed()) && v != *value;
    if commit {
        *value = v;
    }
    commit
}

/// A button of one item's row, its words `key`, named "Delete <owner>" (M25-p5#4). Returns whether it was clicked.
fn delete_button(ui: &mut Ui, key: &str, owner: &str) -> bool {
    let response = components::button(ui, Kind::Secondary, label(strings::Key::run_time(key)), true, 0.0);
    let name = strings::format(strings::Key::run_time("symbol-delete-item"), &[owner]);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &name));
    response.clicked()
}

/// A check box with the words `key`, named `name` for the item it belongs to (M25-p5#4). Returns whether it changed.
fn named_checkbox(ui: &mut Ui, on: &mut bool, key: &str, name: &str) -> bool {
    let response = ui.checkbox(on, label(strings::Key::run_time(key)));
    let state = *on;
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, ui.is_enabled(), state, name));
    response.changed()
}

/// `{part} of {owner}`: the accessible name of a control that belongs to an item (M25-p3#8).
fn owned(part: &str, owner: &str) -> String {
    strings::format(strings::Key::run_time("symbol-owned"), &[part, owner])
}

/// A collapsible section in the house style (doc 16a §6.8): the header component, its count when it lists items.
fn section(ui: &mut Ui, salt: &str, key: &str, count: Option<usize>, default_open: bool) -> bool {
    let id = ui.make_persistent_id(("symbol-section", salt));
    components::section_header(ui, id, label(strings::Key::run_time(key)), count, default_open)
}

/// A path the user types beside its picker: wrapped, so a long path shows whole at every width (SM25#8).
fn path_field(ui: &mut Ui, key: &str, value: &mut String) -> egui::Response {
    let caption = ui.label(label(strings::Key::run_time(key)));
    let response = ui
        .add(
            egui::TextEdit::multiline(value)
                .desired_rows(1)
                .return_key(None)
                .desired_width(ui.available_width()),
        )
        .labelled_by(caption.id);
    // A multi-line field takes its name on its own node.
    let name = label(strings::Key::run_time(key));
    ui.ctx()
        .accesskit_node_builder(response.id, |n| n.set_label(name));
    response
}

/// The words of a potential role as the role list shows them: codes as printed, Signal and Undefined in words.
fn role_words(role: &str) -> String {
    match role {
        "" => "—".to_owned(),
        "signal" | "undefined" => label(strings::Key::run_time(&format!("symbol-role-{role}"))).to_owned(),
        _ => role
            .to_uppercase()
            .replace("L_PLUS", "L+")
            .replace("L_MINUS", "L-"),
    }
}

/// The potential roles a connection point takes, in the role list's order after "None".
const ROLES: [&str; 13] = [
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
];

/// The role the letters typed into an open role list name: the role whose words they are, else the
/// first whose words start with them, in any case; `None` for none.
fn role_typed(typed: &str) -> Option<&'static str> {
    let typed = typed.trim().to_lowercase();
    if typed.is_empty() {
        return None;
    }
    let words = |r: &str| role_words(r).to_lowercase();
    ROLES
        .into_iter()
        .find(|r| words(r) == typed)
        .or_else(|| ROLES.into_iter().find(|r| words(r).starts_with(&typed)))
}

/// A combo box's node says whether its list shows.
trait Expanded {
    fn expanded(self, ui: &Ui) -> Self;
}
impl Expanded for egui::Response {
    fn expanded(self, ui: &Ui) -> Self {
        components::combo_expanded(ui, &self);
        self
    }
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
    shortcut: String,
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
            shortcut,
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

/// The key that starts a new line in a text that may hold one; Enter alone commits.
fn new_line_key() -> egui::KeyboardShortcut {
    egui::KeyboardShortcut::new(egui::Modifiers::SHIFT, egui::Key::Enter)
}

/// Whether Enter without Shift was pressed in the focused field `input`: the commit of a text that may hold a line
/// break.
fn enter_pressed(ui: &Ui, input: egui::Id) -> bool {
    ui.memory(|m| m.has_focus(input))
        && ui.input(|i| {
            i.events.iter().any(|e| {
                matches!(e, egui::Event::Key { key: egui::Key::Enter, pressed: true, modifiers, .. }
                    if !modifiers.shift)
            })
        })
}

// Keep an in-progress empty translation in the widget, committing on Enter/focus loss. A text that may hold a line
// break (`lines`) takes it with Shift+Enter.
fn text_field(ui: &mut Ui, key: &str, value: &mut Value, lines: bool) -> bool {
    let id = ui.make_persistent_id(key);
    let (source, mut buffer) = ui
        .data_mut(|d| d.get_temp::<(Value, String)>(id))
        .filter(|(source, _)| source == value)
        .unwrap_or_else(|| (value.clone(), display_text(value)));
    let caption = ui.label(label(strings::Key::run_time(key)));
    let input = id.with("input");
    let enter = lines && enter_pressed(ui, input);
    let edit = if lines {
        egui::TextEdit::multiline(&mut buffer)
            .desired_rows(1)
            .return_key(new_line_key())
    } else {
        egui::TextEdit::singleline(&mut buffer)
    };
    let response = ui
        .add(edit.id(input).desired_width(ui.available_width()))
        .labelled_by(caption.id);
    if enter {
        response.surrender_focus();
    }
    response.widget_info(|| {
        let mut info = egui::WidgetInfo::labeled(
            egui::Role::TextInput,
            ui.is_enabled(),
            label(strings::Key::run_time(key)),
        );
        info.current_text_value = Some(buffer.clone());
        info
    });
    // M25-p5#2: Escape cancels the entry.
    let escape = response.lost_focus() && cancelled(ui);
    let commit = (response.lost_focus() || enter) && !escape && buffer != display_text(value);
    if escape {
        buffer = display_text(value);
    }
    ui.data_mut(|d| d.insert_temp(id, (source, buffer.clone())));
    if commit {
        edit_text(value, &buffer);
    }
    commit
}

/// Whether a role list is open: the panel then ends in room for the list, so the list opens below its
/// row, which the panel scrolls to its top.
fn role_room_id() -> egui::Id {
    egui::Id::unique("symbol-role-room")
}

fn function_pins(ui: &mut Ui, cps: &mut [Value]) -> bool {
    let t = theme::current(ui.ctx());
    let mut changed = false;
    ui.data_mut(|d| d.insert_temp(role_room_id(), false));
    egui::Grid::new("symbol-function-pins")
        .spacing(Vec2::new(t.space_2, t.space_2))
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
                // M25-p3#4: the designation commits on Enter or focus loss and can be cleared and retyped.
                let name = control_name(
                    label(strings::Key::run_time("symbol-designation")),
                    &(i + 1).to_string(),
                );
                if committed_text(
                    ui,
                    ("pin", i),
                    &name,
                    None,
                    &mut designation,
                    2.0 * t.size_badge,
                    false,
                ) {
                    cp["designation"] = json!(designation);
                    changed = true;
                }
                let role = cp["potential_role"].as_str().unwrap_or("").to_owned();
                // The list opens below its row, which the panel scrolls to its top with room for the
                // list under it, shows every role without scrolling, and letters typed while it is open choose the
                // role they name.
                let button = ui.add(
                    egui::Button::new(role_words(&role)).min_size(Vec2::new(t.size_button_min_width, 0.0)),
                );
                let chevron = egui::Rect::from_center_size(
                    egui::pos2(button.rect.right() - t.space_3, button.rect.center().y),
                    Vec2::splat(t.icon_sm),
                );
                components::combo_chevron(ui, chevron, ui.style().interact(&button), false);
                let list_id = button.id.with("roles");
                egui::Popup::menu(&button)
                    .id(list_id)
                    .align(egui::RectAlign::BOTTOM_START)
                    .align_alternatives(&[])
                    .show(|ui| {
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
                        for role in ROLES {
                            changed |= ui
                                .selectable_value(&mut cp["potential_role"], json!(role), role_words(role))
                                .changed();
                        }
                    });
                let open = egui::Popup::is_id_open(ui.ctx(), list_id);
                ui.ctx()
                    .accesskit_node_builder(button.id, |n| n.set_expanded(open));
                let typed_id = button.id.with("typed");
                if open {
                    ui.data_mut(|d| d.insert_temp(role_room_id(), true));
                    if button.clicked() {
                        button.scroll_to_me(Some(egui::Align::Min));
                        ui.data_mut(|d| d.remove::<String>(typed_id));
                    }
                    let letters: String = ui.input(|i| {
                        i.events
                            .iter()
                            .filter_map(|e| match e {
                                egui::Event::Text(t) => Some(t.as_str()),
                                _ => None,
                            })
                            .collect()
                    });
                    if !letters.is_empty() {
                        let mut typed = ui.data(|d| d.get_temp::<String>(typed_id)).unwrap_or_default();
                        typed.push_str(&letters);
                        if let Some(found) = role_typed(&typed)
                            && cp["potential_role"] != json!(found)
                        {
                            cp["potential_role"] = json!(found);
                            changed = true;
                        }
                        ui.data_mut(|d| d.insert_temp(typed_id, typed));
                    }
                } else {
                    ui.data_mut(|d| d.remove::<String>(typed_id));
                }
                button.widget_info(|| {
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
                arc_angles: [0, 180],
                slot_pin: editor.draft.symbol.cps.iter().next().map_or(1, |p| p.idx),
                function_key: editor
                    .draft
                    .function_ref
                    .as_ref()
                    .map(|r| format!("library:{}:{}", r.library, r.item))
                    .unwrap_or_default(),
                ..Default::default()
            };
        }
        let Some(editor) = shell.symbol_editor() else {
            return;
        };
        // A folder picked or saved to shows in the field; a key that chose a tool lights it.
        if self.seen_directory != editor.directory {
            self.seen_directory.clone_from(&editor.directory);
            self.directory.clone_from(&editor.directory);
        }
        if self.seen_tool != editor.tool {
            self.seen_tool = editor.tool;
            self.tool = ew_viewmodel::shell::symbol_editor::TOOLS
                .iter()
                .position(|t| *t == editor.tool)
                .unwrap_or(0);
            self.start = None;
            self.text_mark = None;
            self.move_start = None;
            self.vertices.clear();
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
                self.tool_button(ui, shell, events, i);
            }
            let more = ribbon_button(
                ui,
                "symbol-more-shapes",
                "category",
                matches!(self.tool, 4 | 5 | 11 | 12),
                true,
                true,
                String::new(),
            );
            egui::Popup::menu(&more).show(|ui| {
                for i in [4, 5, 11, 12] {
                    if self.tool_button(ui, shell, events, i) {
                        ui.close();
                    }
                }
            });
            ui.separator();
            for i in [6, 7, 8, 9, 10] {
                self.tool_button(ui, shell, events, i);
            }
        });
        ui.horizontal(|ui| {
            let save_key = ew_viewmodel::actions::find("file.save")
                .map(|m| shell.shortcut_text(m))
                .unwrap_or_default();
            // A fresh draft, asking first when the kept one has unsaved changes.
            if ribbon_button(
                ui,
                "symbol-new-symbol",
                "file-plus",
                false,
                true,
                false,
                String::new(),
            )
            .clicked()
            {
                ask(events, "symbol.new", json!({"discard": true}));
            }
            // P10: no command switches the panel; an empty folder is refused in words.
            if ribbon_button(ui, "symbol-save", "device-floppy", false, true, false, save_key).clicked() {
                action(events, "symbol.save", json!({"directory":self.directory}));
            }
            if ribbon_button(
                ui,
                "symbol-import",
                "file-import",
                false,
                true,
                false,
                String::new(),
            )
            .clicked()
            {
                ask(events, "symbol.open", self.file_params());
            }
            if ribbon_button(
                ui,
                "symbol-scratch",
                "eye",
                false,
                shell.has_project(),
                false,
                String::new(),
            )
            .clicked()
                && shell.has_project()
            {
                action(events, "symbol.scratch", json!({}));
            }
            // Close leaves the draft and its history kept; Discard draft is the explicit way to drop them.
            if ribbon_button(ui, "symbol-close", "x", false, true, false, String::new()).clicked() {
                action(events, "symbol.close", json!({}));
            }
            if ribbon_button(
                ui,
                "symbol-discard-draft",
                "trash",
                false,
                true,
                false,
                String::new(),
            )
            .clicked()
            {
                ask(events, "symbol.close", json!({"discard": true}));
            }
            ui.separator();
            ui.label(label(strings::Key::run_time(if editor.variant == 0 {
                match self.tool {
                    0 => "symbol-select-hint",
                    3 => "symbol-circle-hint",
                    4 | 5 => "symbol-bounds-hint",
                    9 => "symbol-anchor-hint",
                    10 => "symbol-link-hint",
                    6..=8 => "symbol-point-hint",
                    _ => "symbol-draw-hint",
                }
            } else {
                "symbol-edit-a"
            })));
        });
    }

    /// The file field's path, or none so that the window's file picker asks for it.
    fn file_params(&self) -> Value {
        if self.file.trim().is_empty() {
            json!({})
        } else {
            json!({"file": self.file})
        }
    }

    fn tool_button(
        &mut self,
        ui: &mut Ui,
        shell: &ShellModel,
        events: &mut Vec<Event>,
        index: usize,
    ) -> bool {
        let (word, caption, icon) = TOOLS[index];
        // P8: the tooltip names the key of the keymap that chooses the tool.
        let key = ew_viewmodel::shell::symbol_editor::TOOL_KEYS
            .iter()
            .find(|(tool, _)| *tool == word)
            .and_then(|(_, action)| ew_viewmodel::actions::find(action))
            .map(|m| shell.shortcut_text(m))
            .unwrap_or_default();
        let clicked = ribbon_button(ui, caption, icon, self.tool == index, true, false, key).clicked();
        // P10: choosing a tool leaves the panel as it is.
        if clicked {
            self.tool = index;
            self.start = None;
            self.text_mark = None;
            self.move_start = None;
            self.vertices.clear();
            action(events, "symbol.tool", json!({"tool": word}));
        }
        clicked
    }

    pub fn show(&mut self, ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
        self.sync(shell);
        let Some(editor) = shell.symbol_editor() else {
            return;
        };
        if self.zoom == 0.0 {
            self.zoom = 8.0;
        }
        if self.directory.is_empty() {
            self.directory = editor.directory.clone();
        }
        let draft = &editor.draft;
        let styles = shell.symbol_text_styles();
        let t = theme::current(ui.ctx());
        let view_size = ui.available_size();
        let symbol_name = display_text(&serde_json::to_value(&draft.symbol.name).unwrap_or_default());
        // P3: the saved state and the grid are the status bar's; the header names the symbol and the error, once.
        ui.horizontal(|ui| {
            ui.strong(&symbol_name);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if button(ui, "symbol-fit") {
                    self.manual_view = false;
                    self.fitted = None;
                }
            });
        });
        // P3 (SM25b m2): a refusal that Validation lists, issue by issue, shows there only; any other shows here, once.
        let issues: Vec<String> = draft
            .validate()
            .iter()
            .map(|i| strings::format(strings::Key::run_time(i.code), &[&i.detail]))
            .collect();
        if let Some(error) = &editor.error
            && !error.lines().all(|line| issues.iter().any(|i| i == line))
        {
            ui.colored_label(t.error.color32(), error);
            // A page on another connection grid offers a new page with the symbol's grid. M25-p7#2: a draft
            // saved to a library and unchanged places the published symbol; any other is tested, each under its own
            // command's label.
            let (text, offer) = if editor.published().is_some() {
                ("symbol-place-new-page", "symbol.new_page")
            } else {
                ("symbol-scratch", "symbol.scratch")
            };
            if editor.offer_new_page
                && components::button(
                    ui,
                    Kind::Secondary,
                    label(strings::Key::run_time(text)),
                    shell.has_project(),
                    0.0,
                )
                .clicked()
            {
                action(events, offer, json!({}));
            }
        }
        ui.separator();
        let width = crate::dock::left_default_width(&t, ui.available_width());
        let height = ui.available_height();
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(Vec2::new(width,height),egui::Layout::top_down(egui::Align::Min),|ui| {
                // The scroll bar takes its own room, so no field runs under it.
                ui.spacing_mut().scroll.floating = false;
                egui::ScrollArea::vertical().id_salt("symbol-properties-scroll").max_height(height).show(ui,|ui| {
                    ui.set_min_width(width - t.space_5);
                    let tabs = ["symbol-drawing-tab", "symbol-data-tab", "symbol-library-tab"].map(|k| label(strings::Key::run_time(k)));
                    let choices: Vec<(&str, bool)> = tabs.iter().enumerate().map(|(i, l)| (*l, self.panel == i)).collect();
                    if let Some(i) = crate::tool_view::segmented(ui, label(strings::Key::run_time("symbol-panels")), &choices) {
                        self.panel = i;
                    }
                    ui.separator();
                    // SM25#7: an import with more drawings says so first, with the drawings to choose from.
                    if self.panel == 0 && editor.candidates.len() > 1 {
                        ui.label(strings::format(strings::Key::run_time("symbol-candidates-count"), &[&editor.candidates.len().to_string()]));
                        if section(ui, "candidates", "symbol-candidates", Some(editor.candidates.len()), true) {
                            for (i,d) in editor.candidates.iter().enumerate() {
                                let name = display_text(&serde_json::to_value(&d.symbol.name).unwrap_or_default());
                                if components::button(ui, Kind::Secondary, &name, true, 0.0).clicked() {
                                    ask(events, "symbol.candidate",json!({"index":i}));
                                }
                            }
                        }
                    }
                    if self.panel == 0 && self.tool != 0 { self.numeric_draw(ui, events); }
                    if !draft.import_notes.is_empty() && !draft.import_reviewed { ui.label(label(strings::Key::run_time("symbol-import-review"))); }
                    if !draft.import_notes.is_empty() && section(ui, "import-notes", "symbol-import-notes", Some(draft.import_notes.len() + draft.import_details.len()), false) {
                        for note in &draft.import_notes { ui.label(label(strings::Key::run_time(note))); }
                        for detail in &draft.import_details { ui.label(&detail.message); }
                        let mut reviewed=draft.import_reviewed;
                        if ui.checkbox(&mut reviewed,label(strings::Key::run_time("symbol-reviewed"))).changed() {
                            let mut d=draft.clone();d.import_reviewed=reviewed;replace_action(events,draft,serde_json::to_value(d).unwrap_or_default());
                        }
                    }
                    if self.panel == 1 {
                        self.pin_mapping(ui, draft, events);
                        if section(ui, "assign", "symbol-assign", None, false) {
                            let current = editor.functions.iter().find(|f| f.source == self.function_key);
                            let title = current.map(function_choice_name).unwrap_or_else(|| label(strings::Key::run_time("symbol-choose-function")).to_owned());
                            egui::ComboBox::from_id_salt("symbol-function-choice").selected_text(title).width(ui.available_width()).show_ui(ui, |ui| {
                                for f in &editor.functions {
                                    ui.selectable_value(&mut self.function_key, f.source.clone(), function_choice_name(f)).on_hover_text(&f.key);
                                }
                            }).response.expanded(ui).widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, true, label(strings::Key::run_time("symbol-choose-function"))));
                            if button(ui, "symbol-assign") { action(events, "symbol.function", json!({"key": self.function_key})); }
                        }
                    }
                    ui.push_id(editor.generation, |ui| self.properties(ui,draft,&styles,events));
                    if self.panel == 2 {
                    if section(ui, "fork", "symbol-fork", None, true) {
                        field(ui, "symbol-source-filter", &mut self.source_filter);
                        let title = editor.sources.iter().find(|s| s.source == self.source).map(source_choice_name).unwrap_or_else(|| label(strings::Key::run_time("symbol-choose-source")).to_owned());
                        egui::ComboBox::from_id_salt("symbol-fork-source").selected_text(title).width(ui.available_width()).show_ui(ui, |ui| {
                            for source in &editor.sources {
                                let name = source_choice_name(source);
                                if name.to_lowercase().contains(&self.source_filter.to_lowercase()) || source.key.as_deref().is_some_and(|k| k.to_lowercase().contains(&self.source_filter.to_lowercase())) {
                                    ui.selectable_value(&mut self.source, source.source.clone(), name);
                                }
                            }
                        }).response.expanded(ui).widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, true, label(strings::Key::run_time("symbol-choose-source"))));
                        field(ui, "symbol-fork-key", &mut self.fork_key);
                        if components::button(ui, Kind::Secondary, label(strings::Key::run_time("symbol-fork-create")), !self.source.is_empty() && !self.fork_key.is_empty(), 0.0).clicked() {
                            ask(events, "symbol.fork", json!({"source":self.source,"key":self.fork_key}));
                        }
                    }
                    ui.separator();
                    // M25-p3#11: the typed path beside the house file and folder pickers; an empty file field asks
                    // the picker.
                    if section(ui, "import", "symbol-import", None, false) {
                        path_field(ui,"symbol-file",&mut self.file);
                        ui.horizontal_wrapped(|ui| {
                            if button(ui,"symbol-import") { ask(events, "symbol.open",self.file_params()); }
                            if button(ui,"symbol-export") { action(events,"symbol.export",self.file_params()); }
                        });
                    }
                    // M25-p5#1: the folder typed is the editor's on Enter or focus loss, so Ctrl+S, File ▸ Save symbol
                    // and the quit question save there too; Escape takes the editor's folder back.
                    let folder = path_field(ui,"symbol-library-folder",&mut self.directory);
                    if folder.lost_focus() && self.directory != editor.directory {
                        if cancelled(ui) {
                            self.directory.clone_from(&editor.directory);
                        } else {
                            action(events, "symbol.directory", json!({"directory": self.directory}));
                        }
                    }
                    if button(ui, "symbol-choose-folder") { action(events, "symbol.directory", json!({})); }
                    if section(ui, "place", "symbol-place", None, false) {
                        ui.horizontal(|ui| {
                            for (i, key) in ["symbol-place-x", "symbol-place-y"].iter().enumerate() {
                                let name = label(strings::Key::run_time(key));
                                let caption = ui.label(name);
                                ui.add(egui::DragValue::new(&mut self.place[i]).speed(4.0)).labelled_by(caption.id).widget_info(|| {
                                    let mut info = egui::WidgetInfo::drag_value(true, self.place[i]);
                                    info.label = Some(name.to_owned());
                                    info
                                });
                            }
                        });
                        if components::button(ui, Kind::Secondary, label(strings::Key::run_time("symbol-place")), shell.has_project(), 0.0).clicked() { action(events,"symbol.place",json!({"x":self.place[0],"y":self.place[1]})); }
                        if components::button(ui, Kind::Secondary, label(strings::Key::run_time("symbol-scratch")), shell.has_project(), 0.0).clicked() { action(events,"symbol.scratch",json!({})); }
                    }
                    }
                    ui.separator(); ui.strong(label(strings::Key::run_time("symbol-validation")));
                    // SM25#9/#10: the issues are what to do before saving, in the secondary text; the refusal of an
                    // operation is the header's error, never repeated here.
                    if issues.is_empty() { ui.label(label(strings::Key::run_time("symbol-valid"))); }
                    for issue in &issues { ui.colored_label(t.text_secondary.color32(), issue); }
                    // Room for an open role list below its row.
                    if self.panel == 1 && ui.data(|d| d.get_temp::<bool>(role_room_id())).unwrap_or(false) { ui.add_space((ROLES.len() + 1) as f32 * (t.size_row + t.space_2)); }
                });
            });
            ui.separator();
            ui.vertical(|ui| {
                let tools = TOOLS;
                // The tool's own fields stand in a band of one height above the canvas, the same for
                // every tool, so choosing a tool never moves or shrinks the canvas under the pointer.
                // Its height holds a text of two lines, which the text tool's field may take.
                let band = t.size_control + t.size_row;
                let (band_rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), band), Sense::hover());
                let mut band_ui = ui.new_child(egui::UiBuilder::new().max_rect(band_rect).layout(egui::Layout::left_to_right(egui::Align::Center)));
                band_ui.set_clip_rect(band_rect.intersect(ui.clip_rect()));
                let ui_band = &mut band_ui;
                let field_width = (band_rect.width() / 3.0).max(t.size_button_min_width);
                let mut text_input = None;
                if self.tool == 6 {
                    let (input, entered) = ui_band.allocate_ui_with_layout(Vec2::new(field_width * 2.0, band), egui::Layout::left_to_right(egui::Align::Center), |ui| lines_field(ui,"symbol-text",&mut self.text)).inner;
                    text_input = Some(input);
                    // The text typed after a click on the canvas is created at that click, on Enter.
                    if entered && !self.text.is_empty() && let Some(p) = self.text_mark.take() {
                        action(events,"symbol.draw",json!({"tool":TOOLS[6].0,"x":p[0],"y":p[1],"text":self.text}));
                    }
                }
                if self.tool == 10 { ui_band.allocate_ui_with_layout(Vec2::new(field_width * 2.0, band), egui::Layout::left_to_right(egui::Align::Center), |ui| field(ui,"symbol-name",&mut self.link_name)); }
                if self.tool == 8 { slot_kind_control(ui_band,&mut self.slot_kind,&mut self.slot_pin); }
                if self.tool == 7 {
                    ui_band.allocate_ui_with_layout(Vec2::new(field_width, band), egui::Layout::left_to_right(egui::Align::Center), |ui| field(ui,"symbol-designation",&mut self.designation));
                    egui::ComboBox::from_id_salt("symbol-cp-direction").selected_text(label(strings::Key::run_time(["symbol-up","symbol-right","symbol-down","symbol-left"][self.direction]))).show_ui(ui_band,|ui| {
                        for (i,k) in ["symbol-up","symbol-right","symbol-down","symbol-left"].iter().enumerate() { ui.selectable_value(&mut self.direction,i,label(strings::Key::run_time(k))); }
                    }).response.expanded(ui_band).widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, true, label(strings::Key::run_time("symbol-direction"))));
                }
                if self.tool>=11 {ui_band.label(label(strings::Key::run_time("symbol-path-hint")));}
                // The variant strip under the canvas: its caption row and its thumbnails.
                let strip = 3.0 * t.size_row;
                let canvas_height = (ui.available_height() - strip - t.size_row - 2.0 * t.space_3).max(4.0 * t.size_row);
                let (rect,response) = ui.allocate_exact_size(Vec2::new(ui.available_width(),canvas_height),Sense::click_and_drag());
                let canvas_name = strings::format(strings::Key::run_time("symbol-canvas"), &[&symbol_name]);
                response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Canvas, true, &canvas_name));
                let painter = ui.painter_at(rect);
                painter.rect_filled(rect,t.radius_none,t.canvas_sheet.color32());
                if response.dragged_by(egui::PointerButton::Middle) { self.manual_view=true; self.pan += ui.input(|i|i.pointer.delta()); }
                if response.hovered() {
                    let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                    if scroll != 0.0 {self.manual_view=true;}
                    self.zoom = (self.zoom * (scroll * 0.0015).exp()).clamp(1.0,80.0);
                }
                // Fit fills the canvas at every width (SM25 nit: a small import filled 60 px at 1920). It
                // fits when the draft opens, when the canvas changes size and when Fit is chosen, never because the
                // drawing changed; an empty draft shows the 40 mm square right and below its origin. The
                // view stands on the canvas's bottom edge, which the refusal banner above it does not move.
                if !self.manual_view && self.fitted != Some(view_size) && let Ok(g)=draft.preview(editor.variant) {
                    self.fitted = Some(view_size);
                    let b=g.bounds;let empty=draft.symbol.graphics.is_empty()&&draft.symbol.cps.is_empty();
                    let (width,height,centre)=if empty {(40.0,40.0,Vec2::splat(20.0))} else {
                        ((b.max().x.0-b.min().x.0) as f32/1e6,(b.max().y.0-b.min().y.0) as f32/1e6,Vec2::new((b.min().x.0+b.max().x.0) as f32/2e6,(b.min().y.0+b.max().y.0) as f32/2e6))
                    };
                    self.zoom=(rect.width()/(width+16.0)).min(rect.height()/(height+16.0)).clamp(0.01,80.0);
                    self.pan=-centre*self.zoom-Vec2::new(0.0,rect.height()/2.0);
                }
                let origin = rect.center_bottom() + self.pan;
                let step = self.zoom * draft.symbol.grid.at(1).map_or(4.0,|n|n.0 as f32 / 1_000_000.0);
                if step >= t.space_3 {
                    let x0 = ((rect.left()-origin.x)/step).floor() as i32;
                    let y0 = ((rect.top()-origin.y)/step).floor() as i32;
                    for x in x0..x0+((rect.width()/step).ceil() as i32)+1 { for y in y0..y0+((rect.height()/step).ceil() as i32)+1 {
                        painter.circle_filled(origin+Vec2::new(x as f32*step,y as f32*step),t.border_width,t.canvas_grid_dot.color32());
                    }}
                }
                painter.line_segment([origin-Vec2::new(t.space_3,0.0),origin+Vec2::new(t.space_3,0.0)],Stroke::new(t.border_width,t.canvas_grid_dot.color32()));
                painter.line_segment([origin-Vec2::new(0.0,t.space_3),origin+Vec2::new(0.0,t.space_3)],Stroke::new(t.border_width,t.canvas_grid_dot.color32()));
                paint(&painter,draft,editor.variant,origin,self.zoom,true,&styles);
                for p in self.vertices.windows(2) {painter.line_segment([origin+Vec2::new(p[0][0] as f32,p[0][1] as f32)*self.zoom,origin+Vec2::new(p[1][0] as f32,p[1][1] as f32)*self.zoom],Stroke::new(t.border_width,t.canvas_selection.color32()));}
                if editor.variant != 0 { painter.text(rect.left_top()+Vec2::splat(t.space_3),egui::Align2::LEFT_TOP,label(strings::Key::run_time("symbol-edit-a")),theme::font(&t.type_body),t.canvas_selection.color32()); }
                if editor.variant == 0 && self.tool == 0 {
                    if response.clicked() && let Some(pointer)=response.interact_pointer_pos() {
                        self.selected=pick_graphic(draft,(pointer-origin)/self.zoom,t.space_2/self.zoom,&styles);
                    }
                    // M25-p3#3: a drag moves the shape under its start, which it selects; a drag that starts on no
                    // shape moves nothing.
                    if response.drag_started_by(egui::PointerButton::Primary) {
                        let press = ui.input(|i| i.pointer.press_origin());
                        self.move_start = None;
                        if let Some(start) = press && let Some(index) = pick_graphic(draft,(start-origin)/self.zoom,t.space_2/self.zoom,&styles) {
                            self.selected = Some(index);
                            self.move_start = Some(start);
                        }
                    }
                    if let Some(index)=self.selected && let Some(bounds)=graphic_bounds(draft,index,&styles) {
                        let lo=origin+Vec2::new(bounds[0],bounds[1])*self.zoom;
                        let hi=origin+Vec2::new(bounds[2],bounds[3])*self.zoom;
                        painter.rect_stroke(egui::Rect::from_two_pos(lo,hi).expand(t.space_1),t.radius_none,Stroke::new(t.border_width,t.canvas_selection.color32()),egui::StrokeKind::Outside);
                        if response.drag_stopped_by(egui::PointerButton::Primary) && let Some(start) = self.move_start.take() && let Some(end) = response.interact_pointer_pos() {
                            let delta=(end-start)/self.zoom;
                            let dx=(f64::from(delta.x)*2.0).round() as i64*500_000;
                            let dy=(f64::from(delta.y)*2.0).round() as i64*500_000;
                            if dx!=0||dy!=0 { let mut d=serde_json::to_value(draft).unwrap_or_default();translate_graphic(&mut d["symbol"]["graphics"][index],dx,dy);action(events,"symbol.replace",json!({"document":d})); }
                        }
                    }
                }
                // Press, drag and release draws a rectangle (two clicks still do): the press is its first
                // corner, the release its second, each on the 0.5 mm step.
                let on_step = |pos: Pos2| { let at=(pos-origin)/self.zoom; [(f64::from(at.x)*2.0).round()/2.0,(f64::from(at.y)*2.0).round()/2.0] };
                if editor.variant == 0 && self.tool == 2 {
                    if response.drag_started_by(egui::PointerButton::Primary) && let Some(press) = ui.input(|i| i.pointer.press_origin()) {
                        self.start = Some(on_step(press));
                    }
                    if response.drag_stopped_by(egui::PointerButton::Primary) && let Some(end) = response.interact_pointer_pos() && let Some(start) = self.start.take() {
                        let p = on_step(end);
                        if p != start { action(events,"symbol.draw",json!({"tool":tools[2].0,"x":start[0],"y":start[1],"x2":p[0],"y2":p[1]})); }
                    }
                }
                if let Some(mark) = self.text_mark {
                    painter.circle_stroke(origin+Vec2::new(mark[0] as f32,mark[1] as f32)*self.zoom,t.space_2,Stroke::new(t.focus_width,t.canvas_selection.color32()));
                }
                if let Some(start) = self.start {
                    let at = origin+Vec2::new(start[0] as f32*self.zoom,start[1] as f32*self.zoom);
                    painter.circle_stroke(at,t.space_2,Stroke::new(t.focus_width,t.canvas_selection.color32()));
                    if let Some(pointer) = response.hover_pos() {
                        if self.tool == 2 { painter.rect_stroke(egui::Rect::from_two_pos(at,pointer),t.radius_none,Stroke::new(t.border_width,t.canvas_selection.color32()),egui::StrokeKind::Middle); }
                        else { painter.line_segment([at,pointer],Stroke::new(t.border_width,t.canvas_selection.color32())); }
                    }
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
                    // With the Text field empty a click marks its spot and puts the focus in the field.
                    else if self.tool == 6 && self.text.is_empty() {
                        self.text_mark = Some(p);
                        if let Some(input) = text_input { ui.memory_mut(|m| m.request_focus(input)); }
                    }
                    else if matches!(self.tool,6|9) { self.text_mark = None; action(events,"symbol.draw",json!({"tool":tools[self.tool].0,"x":p[0],"y":p[1],"text":self.text})); }
                    else if self.tool > 0 {
                        if let Some(start)=self.start.take() { action(events,"symbol.draw",json!({"tool":tools[self.tool].0,"x":start[0],"y":start[1],"x2":p[0],"y2":p[1],"start":self.arc_angles[0],"sweep":self.arc_angles[1]})); } else { self.start=Some(p); }
                    }
                }
                // Escape ends a gesture; at rest it returns to Select (the key the Select tooltip names).
                if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE,egui::Key::Escape)) {
                    if self.start.is_none() && self.text_mark.is_none() && self.move_start.is_none() && self.vertices.is_empty() && self.tool != 0 {
                        self.tool = 0;
                        action(events, "symbol.tool", json!({"tool": tools[0].0}));
                    }
                    self.start=None;self.text_mark=None;self.move_start=None;self.vertices.clear();
                }
                ui.horizontal(|ui| {
                    ui.label(label(strings::Key::run_time("symbol-variants")));
                });
                let pins: Vec<String> = draft.symbol.cps.iter().map(|cp| draft.function.cps.get(&cp.fn_cp).map_or_else(|| cp.idx.to_string(), |p| p.designation.to_string())).collect();
                ui.horizontal(|ui| {
                    let w = ((ui.available_width()-7.0*ui.spacing().item_spacing.x)/8.0).max(t.size_control);
                    for variant in 0..8u8 {
                        let (r,response)=ui.allocate_exact_size(Vec2::new(w,strip),Sense::click());
                        // M25-p3#8 (doc 07 §5): a preview is named by the symbol, its variant and its pins.
                        let letter = char::from(b'A'+variant).to_string();
                        let name = if pins.is_empty() {
                            strings::format(strings::Key::run_time("symbol-variant-name-plain"), &[&symbol_name, &letter])
                        } else {
                            strings::format(strings::Key::run_time("symbol-variant-name"), &[&symbol_name, &letter, &pins.join(", ")])
                        };
                        response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, variant == editor.variant, &name));
                        let painter=ui.painter_at(r);
                        painter.rect_filled(r,t.radius_mark,if variant==editor.variant {t.canvas_selection.alpha(24).over(t.canvas_sheet).color32()} else {t.canvas_sheet.color32()});
                        let letter_at = r.left_top()+Vec2::new(t.space_2,t.space_1);
                        let letter_rect = painter.text(letter_at,egui::Align2::LEFT_TOP,&letter,theme::font(&t.type_small),t.canvas_sheet_line.color32());
                        // The drawing, its texts included, fits inside its cell, right of its letter.
                        let area = egui::Rect::from_min_max(Pos2::new(letter_rect.right()+t.space_2,r.top()+t.space_2),r.max-Vec2::splat(t.space_2));
                        if let Some(b) = drawn_bounds(draft,variant,&styles) {
                            let (origin,scale) = variant_view(b,area);
                            paint(&painter,draft,variant,origin,scale,false,&styles);
                        }
                        if response.clicked() { action(events,"symbol.variant",json!({"variant":variant})); }
                    }
                });
            });
        });
    }

    fn numeric_draw(&mut self, ui: &mut Ui, events: &mut Vec<Event>) {
        if section(ui, "numeric-draw", "symbol-numeric-draw", None, true) {
            // M25-p3#8: a two-point shape's first point is its start.
            let keys = if matches!(self.tool, 1..=5) {
                ["symbol-start-x", "symbol-start-y", "symbol-end-x", "symbol-end-y"]
            } else {
                ["symbol-x-mm", "symbol-y-mm", "symbol-end-x", "symbol-end-y"]
            };
            for (i, key) in keys.iter().enumerate() {
                if i < 2 || matches!(self.tool, 1..=5) {
                    ui.horizontal(|ui| {
                        let caption = ui.label(label(strings::Key::run_time(key)));
                        let response = ui
                            .add(egui::DragValue::new(&mut self.draw_coordinates[i]).speed(0.5))
                            .labelled_by(caption.id);
                        response.widget_info(|| {
                            let mut info =
                                egui::WidgetInfo::drag_value(ui.is_enabled(), self.draw_coordinates[i]);
                            info.label = Some(label(strings::Key::run_time(key)).to_owned());
                            info
                        });
                    });
                }
            }
            if self.tool == 5 {
                for (i, key) in ["symbol-arc-start", "symbol-arc-sweep"].iter().enumerate() {
                    ui.horizontal(|ui| {
                        let caption = ui.label(label(strings::Key::run_time(key)));
                        let response = ui
                            .add(
                                egui::DragValue::new(&mut self.arc_angles[i])
                                    .range(-360..=360)
                                    .update_while_editing(false),
                            )
                            .labelled_by(caption.id);
                        response.widget_info(|| {
                            let mut info =
                                egui::WidgetInfo::drag_value(ui.is_enabled(), f64::from(self.arc_angles[i]));
                            info.label = Some(label(strings::Key::run_time(key)).to_owned());
                            info
                        });
                    });
                }
            }
            if button(ui, "symbol-add") {
                let [x, y, x2, y2] = self.draw_coordinates;
                match self.tool {
                    7 => action(
                        events,
                        "symbol.cp",
                        json!({"x":x,"y":y,"designation":self.designation,"direction":(["up","right","down","left"][self.direction])}),
                    ),
                    8 => action(
                        events,
                        "symbol.slot",
                        json!({"x":x,"y":y,"kind":slot_kind(self.slot_kind,self.slot_pin)}),
                    ),
                    10 => action(events, "symbol.link", json!({"x":x,"y":y,"name":self.link_name})),
                    11 | 12 => self.vertices.push([x, y]),
                    _ => action(
                        events,
                        "symbol.draw",
                        json!({"tool":TOOLS[self.tool].0,"x":x,"y":y,"x2":x2,"y2":y2,"text":self.text,"start":self.arc_angles[0],"sweep":self.arc_angles[1]}),
                    ),
                }
            }
            if self.tool >= 11
                && self.vertices.len() >= if self.tool == 12 { 3 } else { 2 }
                && button(ui, "symbol-finish-path")
            {
                action(
                    events,
                    "symbol.path",
                    json!({"points":self.vertices,"closed":self.tool == 12}),
                );
                self.vertices.clear();
            }
        }
    }

    fn pin_mapping(&mut self, ui: &mut Ui, draft: &SymbolDraft, events: &mut Vec<Event>) {
        let t = theme::current(ui.ctx());
        if section(
            ui,
            "pin-mapping",
            "symbol-pin-mapping",
            Some(draft.symbol.cps.len()),
            true,
        ) {
            {
                ui.label(label(strings::Key::run_time("symbol-mapping-help")));
                ui.horizontal_wrapped(|ui| {
                    for pin in draft.function.cps.iter().filter(|p| p.physical) {
                        let name = strings::format(
                            strings::Key::run_time("symbol-function-pin"),
                            &[&function_pin_name(draft, pin.idx)],
                        );
                        let response = ui.add(egui::Button::new(name).sense(Sense::drag()));
                        response.dnd_set_drag_payload(FunctionPinDrag(pin.idx));
                    }
                });
                for cp in &draft.symbol.cps {
                    ui.push_id(("map-cp", cp.idx), |ui| {
                        let name = strings::format(
                            strings::Key::run_time("symbol-mapping-target"),
                            &[&cp.idx.to_string()],
                        );
                        let row = ui
                            .horizontal(|ui| {
                                let target = components::button(ui, Kind::Secondary, &name, true, 0.0);
                                let current = function_pin_name(draft, cp.fn_cp);
                                egui::ComboBox::from_id_salt("mapping")
                                    .selected_text(current)
                                    .width(t.size_button_min_width)
                                    .show_ui(ui, |ui| {
                                        for pin in draft
                                            .function
                                            .cps
                                            .iter()
                                            .filter(|p| p.physical && p.discipline == cp.discipline)
                                        {
                                            if ui
                                                .selectable_label(
                                                    cp.fn_cp == pin.idx,
                                                    function_pin_name(draft, pin.idx),
                                                )
                                                .clicked()
                                            {
                                                action(
                                                    events,
                                                    "symbol.map_cp",
                                                    json!({"cp":cp.idx,"fn_cp":pin.idx}),
                                                );
                                            }
                                        }
                                    })
                                    .response
                                    .expanded(ui)
                                    .widget_info(|| {
                                        egui::WidgetInfo::labeled(
                                            egui::Role::ComboBox,
                                            true,
                                            strings::format(
                                                strings::Key::run_time("symbol-map-control"),
                                                &[&cp.idx.to_string()],
                                            ),
                                        )
                                    });
                                target
                            })
                            .inner;
                        if row.dnd_hover_payload::<FunctionPinDrag>().is_some() {
                            ui.painter().rect_stroke(
                                row.rect,
                                theme::radius(t.radius_md),
                                theme::stroke(t.focus_width, t.focus),
                                egui::StrokeKind::Inside,
                            );
                        }
                        if let Some(pin) = row.dnd_release_payload::<FunctionPinDrag>() {
                            action(events, "symbol.map_cp", json!({"cp":cp.idx,"fn_cp":pin.0}));
                        }
                    });
                }
            }
        }
    }

    fn properties(
        &mut self,
        ui: &mut Ui,
        draft: &SymbolDraft,
        styles: &symbols::TextStyleTable,
        events: &mut Vec<Event>,
    ) {
        let Ok(mut doc) = serde_json::to_value(draft) else {
            return;
        };
        let mut changed = false;
        if self.panel == 0
            && !ui.ctx().text_edit_focused()
            && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Delete))
            && let Some(index) = self.selected.take()
            && let Some(graphics) = doc["symbol"]["graphics"].as_array_mut()
            && index < graphics.len()
        {
            graphics.remove(index);
            changed = true;
        }
        changed |= text_field(ui, "symbol-name", &mut doc["symbol"]["name"], false);
        if self.panel == 2 {
            for (key, path) in [("symbol-key", "key"), ("symbol-version", "version")] {
                let mut value = doc[path].as_str().unwrap_or_default().to_owned();
                if doc_field(ui, key, &mut value) {
                    doc[path] = json!(value);
                    changed = true;
                }
            }
            if section(ui, "properties", "symbol-properties", None, false) {
                for key in ["licence", "sources"] {
                    let mut value = if key == "sources" {
                        doc["symbol"]["meta"][key][0]
                            .as_str()
                            .unwrap_or_default()
                            .to_owned()
                    } else {
                        doc["symbol"]["meta"][key].as_str().unwrap_or_default().to_owned()
                    };
                    if doc_field(
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
            }
        }
        let graphic_title = |i: usize| {
            let value = draft
                .symbol
                .graphics
                .get(i)
                .and_then(|g| serde_json::to_value(g).ok())
                .unwrap_or_default();
            let kind = value
                .as_object()
                .and_then(|m| m.keys().next())
                .map(String::as_str)
                .unwrap_or("");
            strings::format(
                strings::Key::run_time("symbol-graphic-item"),
                &[
                    label(strings::Key::run_time(&format!(
                        "symbol-{}",
                        if kind == "rect" { "rectangle" } else { kind }
                    ))),
                    &(i + 1).to_string(),
                ],
            )
        };
        if self.panel == 0
            && section(
                ui,
                "graphics",
                "symbol-graphics",
                Some(draft.symbol.graphics.len()),
                true,
            )
        {
            {
                for i in 0..draft.symbol.graphics.len() {
                    if ui
                        .selectable_label(self.selected == Some(i), graphic_title(i))
                        .clicked()
                    {
                        self.selected = Some(i);
                    }
                }
                if let Some(i) = self.selected
                    && let Some(value) = doc["symbol"]["graphics"].get_mut(i)
                {
                    let owner = graphic_title(i);
                    changed |= numeric_fields(ui, value, "", false, &owner);
                    for kind in ["rect", "circle", "ellipse", "polygon"] {
                        if let Some(shape) = value.get_mut(kind) {
                            let mut filled = !shape["fill"].is_null();
                            if named_checkbox(
                                ui,
                                &mut filled,
                                "symbol-fill",
                                &owned(label(strings::Key::run_time("symbol-fill")), &owner),
                            ) {
                                if filled {
                                    shape["fill"] = json!("by_layer");
                                } else if let Some(object) = shape.as_object_mut() {
                                    object.remove("fill");
                                }
                                changed = true;
                            }
                        }
                    }
                    if let Some(text) = value.get_mut("text") {
                        changed |= text_style_control(ui, text, styles, &owner);
                        changed |= align_control(ui, text, &owner);
                        changed |= ui
                            .push_id(("graphic-text", i), |ui| {
                                text_field(ui, "symbol-text", &mut text["text"], true)
                            })
                            .inner;
                        changed |= rotation_control(ui, text, &owner);
                    }
                    if let Some(arc) = value.get_mut("arc") {
                        for key in ["start", "sweep"] {
                            let mut degrees = arc[key].as_f64().unwrap_or(0.0) / 1000.0;
                            ui.horizontal(|ui| {
                                let caption = label(strings::Key::run_time(if key == "start" {
                                    "symbol-arc-start"
                                } else {
                                    "symbol-arc-sweep"
                                }));
                                ui.label(caption);
                                if number(ui, &mut degrees, 1.0, -360.0..=360.0, &owned(caption, &owner)) {
                                    arc[key] = json!((degrees * 1000.0).round() as i32);
                                    changed = true;
                                }
                            });
                        }
                    }
                    if delete_button(ui, "symbol-delete", &owner) {
                        if let Some(a) = doc["symbol"]["graphics"].as_array_mut() {
                            a.remove(i);
                        }
                        self.selected = None;
                        changed = true;
                    }
                }
            }
        }
        if self.panel == 1 {
            ui.label(label(strings::Key::run_time("symbol-data-help")));
            if section(ui, "function", "symbol-function", None, true) {
                {
                    ui.add_enabled_ui(draft.function_ref.is_none(), |ui| {
                        ui.label(label(strings::Key::run_time("symbol-function-kind")));
                        let kind = doc["function"]["contact_kind"].as_str().unwrap_or("").to_owned();
                        let enabled = ui.is_enabled();
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
                            })
                            .response
                            .expanded(ui)
                            .widget_info(|| {
                                egui::WidgetInfo::labeled(
                                    egui::Role::ComboBox,
                                    enabled,
                                    label(strings::Key::run_time("symbol-function-kind")),
                                )
                            });
                        if let Some(cps) = doc["function"]["cps"].as_array_mut() {
                            changed |= function_pins(ui, cps);
                        }
                    });
                }
            }
            if section(ui, "pins", "symbol-pins", Some(draft.symbol.cps.len()), false) {
                {
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
                                ui.strong(&name);
                                let owner = strings::format(
                                    strings::Key::run_time("symbol-mapping-target"),
                                    &[&name],
                                );
                                changed |= numeric_fields(ui, &mut cp["pos"], "", true, &owner);
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
                                    })
                                    .response
                                    .expanded(ui)
                                    .widget_info(|| {
                                        egui::WidgetInfo::labeled(
                                            egui::Role::ComboBox,
                                            true,
                                            owned(label(strings::Key::run_time("symbol-direction")), &owner),
                                        )
                                    });
                                if delete_button(ui, "symbol-delete-row", &owner) {
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
                }
            }
        }
        if self.panel == 0 {
            if section(
                ui,
                "slots",
                "symbol-slots",
                Some(draft.symbol.text_slots.len()),
                false,
            ) {
                let mut remove = None;
                if let Some(slots) = doc["symbol"]["text_slots"].as_array_mut() {
                    for (i, slot) in slots.iter_mut().enumerate() {
                        ui.push_id(("slot", i), |ui| {
                            let kind = slot["kind"].as_str().unwrap_or("cp_designation");
                            let key = match kind {
                                "tag" => "symbol-tag",
                                "function_text" => "symbol-function-text",
                                "tech_data" => "symbol-tech-data",
                                "xref" => "symbol-xref",
                                _ => "symbol-cp-designation",
                            };
                            // M25-p5#4: each slot is named by its kind and its place in the list.
                            let owner = strings::format(
                                strings::Key::run_time("symbol-graphic-item"),
                                &[label(strings::Key::run_time(key)), &(i + 1).to_string()],
                            );
                            ui.label(&owner);
                            changed |= text_style_control(ui, slot, styles, &owner);
                            changed |= numeric_fields(ui, &mut slot["pos"], "", true, &owner);
                            changed |= rotation_control(ui, slot, &owner);
                            let mut visible = slot["visible"].as_bool().unwrap_or(false);
                            if named_checkbox(
                                ui,
                                &mut visible,
                                "symbol-visible",
                                &strings::format(strings::Key::run_time("symbol-visible-of"), &[&owner]),
                            ) {
                                slot["visible"] = json!(visible);
                                changed = true;
                            }
                            if delete_button(ui, "symbol-delete-row", &owner) {
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
            }
            if section(
                ui,
                "links",
                "symbol-link",
                Some(draft.symbol.link_points.len()),
                false,
            ) {
                let mut remove = None;
                if let Some(points) = doc["symbol"]["link_points"].as_array_mut() {
                    for (i, p) in points.iter_mut().enumerate() {
                        ui.push_id(("link", i), |ui| {
                            let mut s = p["name"].as_str().unwrap_or_default().to_owned();
                            // M25-p5#4: a link point is named by its place in the list, whatever its name.
                            let owner = strings::format(
                                strings::Key::run_time("symbol-graphic-item"),
                                &[label(strings::Key::run_time("symbol-link")), &(i + 1).to_string()],
                            );
                            let caption = label(strings::Key::run_time("symbol-name"));
                            ui.label(caption);
                            let width = ui.available_width();
                            if committed_text(
                                ui,
                                ("doc", "symbol-name"),
                                &owned(caption, &owner),
                                None,
                                &mut s,
                                width,
                                true,
                            ) {
                                p["name"] = json!(s);
                                changed = true;
                            }
                            changed |= numeric_fields(ui, &mut p["pos"], "", true, &owner);
                            if delete_button(ui, "symbol-delete-row", &owner) {
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
            }
            // The cross-reference anchor of its own stands in a list with its place and Delete, which
            // returns the anchor to the origin.
            let named = doc["symbol"]["anchor"].get("named").is_some();
            if section(ui, "anchor", "symbol-anchor", Some(usize::from(named)), false) && named {
                let owner = label(strings::Key::run_time("symbol-anchor")).to_owned();
                ui.push_id("anchor", |ui| {
                    ui.label(&owner);
                    changed |= numeric_fields(ui, &mut doc["symbol"]["anchor"]["named"], "", true, &owner);
                    if delete_button(ui, "symbol-delete-row", &owner) {
                        doc["symbol"]["anchor"] = json!("origin");
                        changed = true;
                    }
                });
            }
        }
        if changed {
            replace_action(events, draft, doc);
        }
        if self.panel == 2 && section(ui, "advanced", "symbol-advanced", None, false) {
            {
                if self.advanced.is_empty() {
                    self.advanced = serde_json::to_string_pretty(draft).unwrap_or_default();
                }
                let advanced = ui.add(
                    egui::TextEdit::multiline(&mut self.advanced)
                        .code_editor()
                        .desired_rows(12)
                        .desired_width(ui.available_width()),
                );
                // M25-p3#8: the document field is named.
                let name = label(strings::Key::run_time("symbol-advanced"));
                ui.ctx()
                    .accesskit_node_builder(advanced.id, |n| n.set_label(name));
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
                        theme::current(ui.ctx()).error.color32(),
                        label(strings::Key::run_time("symbol-json-invalid")),
                    );
                }
            }
        }
    }
}

/// The coordinates of `value` (a shape, a point or a position) as fields named by their part and `owner`: "Start x
/// (mm) of Line 1", "Radius (mm) of Circle 2" (M25-p3#8). Each commits on Enter, focus loss or a drag's end.
fn numeric_fields(ui: &mut Ui, value: &mut Value, part: &str, coordinate: bool, owner: &str) -> bool {
    let mut changed = false;
    match value {
        Value::Object(m) => {
            for (key, v) in m {
                let part = match key.as_str() {
                    "from" => Some("symbol-part-start"),
                    "to" => Some("symbol-part-end"),
                    "centre" => Some("symbol-part-centre"),
                    "r" => Some("symbol-part-radius"),
                    "rx" => Some("symbol-part-radius-x"),
                    "ry" => Some("symbol-part-radius-y"),
                    "rect" | "points" | "pos" => Some(""),
                    _ => None,
                };
                if part.is_some() || v.is_object() {
                    let part = part.map_or(String::new(), |k| {
                        if k.is_empty() {
                            String::new()
                        } else {
                            label(strings::Key::run_time(k)).to_owned()
                        }
                    });
                    let list = matches!(key.as_str(), "rect" | "points");
                    ui.push_id(key, |ui| {
                        if list && let Value::Array(a) = v {
                            for (i, point) in a.iter_mut().enumerate() {
                                let part = strings::format(
                                    strings::Key::run_time(if key == "rect" {
                                        "symbol-part-corner"
                                    } else {
                                        "symbol-part-point"
                                    }),
                                    &[&(i + 1).to_string()],
                                );
                                ui.push_id(i, |ui| {
                                    changed |= numeric_fields(ui, point, &part, true, owner);
                                });
                            }
                        } else {
                            changed |= numeric_fields(ui, v, &part, true, owner);
                        }
                    });
                }
            }
        }
        Value::Array(a) => {
            for (i, v) in a.iter_mut().enumerate() {
                ui.push_id(i, |ui| {
                    let axis = if i == 0 { "x" } else { "y" };
                    let caption = if part.is_empty() {
                        label(strings::Key::run_time(if i == 0 {
                            "symbol-x-mm"
                        } else {
                            "symbol-y-mm"
                        }))
                        .to_owned()
                    } else {
                        strings::format(strings::Key::run_time("symbol-axis-mm"), &[part, axis])
                    };
                    changed |= coordinate_field(ui, v, &caption, owner);
                });
            }
        }
        Value::Number(_) if coordinate => {
            let caption = strings::format(strings::Key::run_time("symbol-mm"), &[part]);
            changed |= coordinate_field(ui, value, &caption, owner);
        }
        _ => {}
    }
    changed
}

/// One millimetre field of a coordinate in nanometres, shown as `caption` and named `caption of owner`.
fn coordinate_field(ui: &mut Ui, value: &mut Value, caption: &str, owner: &str) -> bool {
    let Some(nm) = value.as_f64() else {
        return false;
    };
    let mut mm = nm / 1_000_000.0;
    let changed = ui
        .horizontal(|ui| {
            ui.label(caption);
            number(
                ui,
                &mut mm,
                0.5,
                -1_000_000.0..=1_000_000.0,
                &owned(caption, owner),
            )
        })
        .inner;
    if changed {
        *value = json!((mm * 1_000_000.0).round() as i64);
    }
    changed
}

/// A text of the document in a labelled field, committed on Enter or focus loss (M25-p3#4).
fn doc_field(ui: &mut Ui, key: &str, value: &mut String) -> bool {
    let name = label(strings::Key::run_time(key));
    let width = ui.available_width();
    committed_text(ui, ("doc", key), name, Some(name), value, width, true)
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
    let list = symbols::preview_display_list(draft, &g, styles, &slot_placeholder);
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
            // A cross-reference anchor of its own shows as a marker: its circle crossed, and its name.
            if matches!(draft.symbol.anchor, symbols::schema::AnchorPolicy::Named(_)) {
                let p = at(g.anchor);
                let stroke = Stroke::new(t.border_width, t.canvas_snap.color32());
                painter.line_segment([p - Vec2::new(9.0, 0.0), p + Vec2::new(9.0, 0.0)], stroke);
                painter.line_segment([p - Vec2::new(0.0, 9.0), p + Vec2::new(0.0, 9.0)], stroke);
                painter.text(
                    p + Vec2::new(8.0, -8.0),
                    egui::Align2::LEFT_BOTTOM,
                    label(strings::Key::run_time("symbol-anchor")),
                    theme::font(&t.type_small),
                    t.canvas_snap.color32(),
                );
            }
        }
    }
}

/// SM25#11: a slot previews as the words of its kind, not one placeholder for all.
fn slot_placeholder(kind: &symbols::schema::SlotKind) -> String {
    label(strings::Key::run_time(match kind {
        symbols::schema::SlotKind::FunctionText => "symbol-function-text",
        symbols::schema::SlotKind::TechData => "symbol-tech-data",
        symbols::schema::SlotKind::Xref => "symbol-xref",
        _ => "symbol-text",
    }))
    .to_owned()
}

/// The bounds of variant `variant` in millimetres as [`paint`] draws it, its texts included.
fn drawn_bounds(draft: &SymbolDraft, variant: u8, styles: &symbols::TextStyleTable) -> Option<[f32; 4]> {
    let g = draft.preview(variant).ok()?;
    let b = symbols::preview_display_list(draft, &g, styles, &slot_placeholder)
        .bounds()
        .unwrap_or(g.bounds);
    Some([
        b.min().x.0 as f32 / 1e6,
        b.min().y.0 as f32 / 1e6,
        b.max().x.0 as f32 / 1e6,
        b.max().y.0 as f32 / 1e6,
    ])
}

/// The origin and the scale that show `bounds` (millimetres) whole and centred in `area`; a drawing smaller than 8 mm
/// shows at the scale of 8 mm.
fn variant_view(bounds: [f32; 4], area: egui::Rect) -> (Pos2, f32) {
    let (width, height) = (bounds[2] - bounds[0], bounds[3] - bounds[1]);
    let scale = (area.width() / width.max(8.0)).min(area.height() / height.max(8.0));
    let centre = Vec2::new(bounds[0] + bounds[2], bounds[1] + bounds[3]) / 2.0;
    (area.center() - centre * scale, scale)
}

fn text_style_control(ui: &mut Ui, value: &mut Value, styles: &symbols::TextStyleTable, owner: &str) -> bool {
    let before = value
        .get("style")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let mut selected = before.clone();
    let t = theme::current(ui.ctx());
    // Each style by its name in words and its size ("Pin designation · 1.85 mm").
    let name = |key: &str| {
        let words = ew_viewmodel::shell::symbol_editor::text_style_words(key);
        match styles.rows().iter().find(|r| r.key == key) {
            Some(row) => strings::format(
                strings::Key::run_time("symbol-style-choice"),
                &[
                    &words,
                    &strings::decimal(&format!("{:.2}", row.em.0 as f64 / 1_000_000.0)),
                ],
            ),
            None => words,
        }
    };
    ui.horizontal(|ui| {
        ui.label(label(strings::Key::run_time("symbol-text-style")));
        egui::ComboBox::from_id_salt("text-style")
            .height(t.size_chooser_rows as f32 * t.size_row)
            .selected_text(if selected.is_empty() {
                label(strings::Key::run_time("symbol-style-default")).to_owned()
            } else {
                name(&selected)
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut selected,
                    String::new(),
                    label(strings::Key::run_time("symbol-style-default")),
                );
                for row in styles.rows() {
                    ui.selectable_value(&mut selected, row.key.clone(), name(&row.key));
                }
            })
            .response
            .expanded(ui)
            .widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::Role::ComboBox,
                    true,
                    owned(label(strings::Key::run_time("symbol-text-style")), owner),
                )
            });
    });
    if selected == before {
        return false;
    }
    if selected.is_empty() {
        if let Some(object) = value.as_object_mut() {
            object.remove("style");
        }
    } else {
        value["style"] = json!(selected);
    }
    true
}

/// The nine alignments of a text, by (horizontal, vertical) position: left, centre, right; top, middle, bottom.
const ALIGNS: [[&str; 3]; 3] = [
    ["top_left", "middle_left", "bottom_left"],
    ["top_centre", "centre", "bottom_centre"],
    ["top_right", "middle_right", "bottom_right"],
];

/// The (horizontal, vertical) position of an alignment word; a word of a newer format reads as top left.
fn align_position(word: &str) -> (usize, usize) {
    (0..3)
        .flat_map(|h| (0..3).map(move |v| (h, v)))
        .find(|(h, v)| ALIGNS[*h][*v] == word)
        .unwrap_or((0, 0))
}

/// A text's alignment as two choices, left, centre or right and top, middle or bottom, named for the
/// text `owner`. Returns whether it changed.
fn align_control(ui: &mut Ui, value: &mut Value, owner: &str) -> bool {
    let (mut h, mut v) = align_position(value["align"].as_str().unwrap_or("top_left"));
    let before = (h, v);
    ui.horizontal(|ui| {
        ui.label(label(strings::Key::run_time("symbol-alignment")));
        for (salt, caption, words, at) in [
            (
                "align-h",
                "symbol-align-horizontal",
                ["symbol-align-left", "symbol-align-centre", "symbol-align-right"],
                &mut h,
            ),
            (
                "align-v",
                "symbol-align-vertical",
                ["symbol-align-top", "symbol-align-middle", "symbol-align-bottom"],
                &mut v,
            ),
        ] {
            egui::ComboBox::from_id_salt(salt)
                .selected_text(label(strings::Key::run_time(words[*at])))
                .show_ui(ui, |ui| {
                    for (i, word) in words.iter().enumerate() {
                        ui.selectable_value(at, i, label(strings::Key::run_time(word)));
                    }
                })
                .response
                .expanded(ui)
                .widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::Role::ComboBox,
                        true,
                        owned(label(strings::Key::run_time(caption)), owner),
                    )
                });
        }
    });
    if (h, v) == before {
        return false;
    }
    value["align"] = json!(ALIGNS[h][v]);
    true
}

fn rotation_control(ui: &mut Ui, value: &mut Value, owner: &str) -> bool {
    let mut degrees = value["rot"].as_f64().unwrap_or(0.0) / 1000.0;
    ui.horizontal(|ui| {
        let caption = label(strings::Key::run_time("symbol-rotation"));
        ui.label(caption);
        if number(ui, &mut degrees, 1.0, -360.0..=360.0, &owned(caption, owner)) {
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
        })
        .response
        .expanded(ui)
        .widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::Role::ComboBox,
                true,
                label(strings::Key::run_time("symbol-slot-kind")),
            )
        });
    if *index == 3 {
        let caption = ui.label(label(strings::Key::run_time("symbol-slot-pin-number")));
        let response = ui.add(egui::DragValue::new(pin)).labelled_by(caption.id);
        response.widget_info(|| {
            let mut info = egui::WidgetInfo::drag_value(true, f64::from(*pin));
            info.label = Some(label(strings::Key::run_time("symbol-slot-pin-number")).to_owned());
            info
        });
    }
}
/// The bounds of graphic `index` in millimetres: as drawn, a text's glyphs included, else as the preview
/// bounds it.
fn graphic_bounds(draft: &SymbolDraft, index: usize, styles: &symbols::TextStyleTable) -> Option<[f32; 4]> {
    let mut d = draft.clone();
    d.symbol.graphics = vec![d.symbol.graphics.get(index)?.clone()];
    d.symbol.cps = Default::default();
    d.symbol.text_slots.clear();
    d.symbol.link_points.clear();
    let g = d.preview(0).ok()?;
    let b = symbols::preview_display_list(&d, &g, styles, &|_| String::new())
        .bounds()
        .unwrap_or(g.bounds);
    Some([
        b.min().x.0 as f32 / 1e6,
        b.min().y.0 as f32 / 1e6,
        b.max().x.0 as f32 / 1e6,
        b.max().y.0 as f32 / 1e6,
    ])
}
/// The graphic under `p`: the smallest whose bounds (widened by `slop`) hold it, so a text is picked before the
/// rectangle behind it; of equally small ones the topmost.
fn pick_graphic(draft: &SymbolDraft, p: Vec2, slop: f32, styles: &symbols::TextStyleTable) -> Option<usize> {
    (0..draft.symbol.graphics.len())
        .rev()
        .filter_map(|i| {
            let b = graphic_bounds(draft, i, styles)?;
            let under = p.x >= b[0] - slop && p.y >= b[1] - slop && p.x <= b[2] + slop && p.y <= b[3] + slop;
            under.then_some(((b[2] - b[0]) * (b[3] - b[1]), i))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, i)| i)
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
            // M25-p3#3: a primitive's tag (`rect` of a rectangle) holds an object; only an array is points.
            if v.is_array() && ["from", "to", "centre", "pos", "rect", "points"].contains(&k.as_str()) {
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
    fn simultaneous_name_and_import_review_commits_are_merged_before_save() {
        let draft = SymbolDraft::new("example.text", "Before").unwrap();
        let mut reviewed = serde_json::to_value(&draft).unwrap();
        reviewed["import_reviewed"] = json!(true);
        let mut renamed = serde_json::to_value(&draft).unwrap();
        renamed["symbol"]["name"] = json!({"en":"After"});
        let mut events = vec![Event::Action("symbol.save", json!({}))];
        replace_action(&mut events, &draft, reviewed);
        replace_action(&mut events, &draft, renamed);
        let Event::Action("symbol.replace", params) = &events[0] else {
            panic!("commit first")
        };
        let changed = SymbolDraft::parse(&serde_json::to_vec(&params["document"]).unwrap()).unwrap();
        assert!(changed.import_reviewed);
        assert_eq!(
            serde_json::to_value(changed.symbol.name).unwrap(),
            json!({"en":"After"})
        );
        assert!(matches!(&events[1], Event::Action("symbol.save", _)));
    }

    /// M25-p5#3: a drag of one point per frame moves a number (egui keeps the precise value between frames), and the
    /// number commits once, at the drag's end.
    #[test]
    fn a_slow_drag_of_a_number_moves_it_and_commits_once_at_its_end() {
        let ctx = egui::Context::default();
        let mut value = 0.0_f64;
        let mut commits = 0;
        let start = Pos2::new(20.0, 16.0);
        let frame = |n: usize, events: Vec<egui::Event>, value: &mut f64, commits: &mut usize| {
            let input = egui::RawInput {
                events,
                time: Some(n as f64 / 60.0),
                screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 200.0))),
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                if number(ui, value, 0.5, -100.0..=100.0, "x") {
                    *commits += 1;
                }
            });
            output.textures_delta.clear();
        };
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(
            0,
            vec![egui::Event::PointerMoved(start)],
            &mut value,
            &mut commits,
        );
        frame(1, vec![button(start, true)], &mut value, &mut commits);
        for i in 1..=40 {
            let at = start + Vec2::new(i as f32, 0.0);
            frame(
                1 + i,
                vec![egui::Event::PointerMoved(at)],
                &mut value,
                &mut commits,
            );
        }
        assert_eq!((value, commits), (0.0, 0), "nothing commits while the drag lasts");
        frame(
            42,
            vec![button(start + Vec2::new(40.0, 0.0), false)],
            &mut value,
            &mut commits,
        );
        assert_eq!(commits, 1);
        assert!(value >= 10.0, "a slow drag stalls: {value}");
    }

    /// In a text's field Shift+Enter starts a new line and Enter alone commits the text with it.
    #[test]
    fn shift_enter_breaks_a_texts_line_and_enter_commits_it() {
        let ctx = egui::Context::default();
        let mut value = json!({"en": "A"});
        let mut commits = 0;
        let key = |key, shift| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: if shift {
                egui::Modifiers::SHIFT
            } else {
                egui::Modifiers::NONE
            },
        };
        let frame = |n: usize, events: Vec<egui::Event>, value: &mut Value, commits: &mut usize| {
            let input = egui::RawInput {
                events,
                time: Some(n as f64 / 60.0),
                screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 200.0))),
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                if n == 0 {
                    let input = ui.make_persistent_id("symbol-text").with("input");
                    ui.memory_mut(|m| m.request_focus(input));
                }
                if text_field(ui, "symbol-text", value, true) {
                    *commits += 1;
                }
            });
            output.textures_delta.clear();
        };
        frame(0, Vec::new(), &mut value, &mut commits);
        frame(1, vec![key(egui::Key::End, false)], &mut value, &mut commits);
        frame(2, vec![key(egui::Key::Enter, true)], &mut value, &mut commits);
        frame(3, vec![egui::Event::Text("B".into())], &mut value, &mut commits);
        assert_eq!(
            (value.clone(), commits),
            (json!({"en": "A"}), 0),
            "nothing commits while typing"
        );
        frame(4, vec![key(egui::Key::Enter, false)], &mut value, &mut commits);
        assert_eq!((value, commits), (json!({"en": "A\nB"}), 1));
    }

    /// A click on a text's glyphs, drawn before the rectangle around it, picks the text, the smaller item;
    /// a click inside the rectangle elsewhere picks the rectangle.
    #[test]
    fn a_click_picks_the_smallest_item_under_the_pointer() {
        let styles = ew_viewmodel::ew_api::output::new_project_text_style_table();
        let draft = SymbolDraft::new("example.pick", "Pick").unwrap();
        let mut doc = serde_json::to_value(&draft).unwrap();
        doc["symbol"]["graphics"] = json!([
            {"text": {"pos": [10_000_000, 10_000_000], "text": {"en": "K1"}, "align": "centre", "style": "cp.small"}},
            {"rect": {"rect": [[0, 0], [20_000_000, 20_000_000]], "stroke": {"pen": "p026"}}},
        ]);
        let draft = SymbolDraft::parse(&serde_json::to_vec(&doc).unwrap()).unwrap();
        let text = graphic_bounds(&draft, 0, &styles).unwrap();
        assert!(text[2] - text[0] > 1.0, "the glyphs' width: {text:?}");
        // On the glyphs, away from the text's anchor point by more than the slop.
        let on_glyphs = Vec2::new(text[2] - 0.2, 10.0);
        assert_eq!(pick_graphic(&draft, on_glyphs, 0.1, &styles), Some(0));
        assert_eq!(pick_graphic(&draft, Vec2::new(3.0, 17.0), 0.1, &styles), Some(1));
        assert_eq!(pick_graphic(&draft, Vec2::new(30.0, 30.0), 0.1, &styles), None);
    }

    /// Every variant, its large two-line text included, shows whole inside its cell, right of its
    /// letter; the text's glyphs reach past the bounds of the geometry alone, which the strip fitted before.
    #[test]
    fn each_variant_with_its_texts_fits_inside_its_cell_right_of_its_letter() {
        let styles = ew_viewmodel::ew_api::output::new_project_text_style_table();
        let draft = SymbolDraft::new("example.strip", "Strip").unwrap();
        let mut doc = serde_json::to_value(&draft).unwrap();
        doc["symbol"]["graphics"] = json!([
            {"rect": {"rect": [[0, 0], [24_000_000, 16_000_000]], "stroke": {"pen": "p026"}}},
            {"text": {"pos": [12_000_000, 0], "text": {"en": "Line 1\nLine 2"}, "align": "centre",
                "style": "section.subtitle"}},
        ]);
        let draft = SymbolDraft::parse(&serde_json::to_vec(&doc).unwrap()).unwrap();
        let geometry = draft.preview(0).unwrap().bounds;
        let drawn = drawn_bounds(&draft, 0, &styles).unwrap();
        assert!(
            drawn[1] < geometry.min().y.0 as f32 / 1e6,
            "the text reaches above the rectangle: {drawn:?}"
        );
        let cell = egui::Rect::from_min_size(Pos2::new(100.0, 700.0), Vec2::new(110.0, 84.0));
        let letter = egui::Rect::from_min_size(cell.min + Vec2::new(8.0, 4.0), Vec2::new(8.0, 14.0));
        let area = egui::Rect::from_min_max(
            Pos2::new(letter.right() + 8.0, cell.top() + 8.0),
            cell.max - Vec2::splat(8.0),
        );
        for variant in 0..8u8 {
            let b = drawn_bounds(&draft, variant, &styles).unwrap();
            let (origin, scale) = variant_view(b, area);
            let shown = egui::Rect::from_min_max(
                origin + Vec2::new(b[0], b[1]) * scale,
                origin + Vec2::new(b[2], b[3]) * scale,
            );
            assert!(
                area.expand(0.01).contains_rect(shown) && !shown.intersects(letter),
                "variant {variant}: {shown:?} in {area:?}"
            );
        }
    }

    /// Letters typed into an open role list choose the role whose words they are, else the first they
    /// begin, in any case.
    #[test]
    fn letters_typed_into_a_role_list_choose_the_role_they_name() {
        for (typed, role) in [
            ("l", Some("l")),
            ("L1", Some("l1")),
            ("p", Some("pe")),
            ("pen", Some("pen")),
            ("l+", Some("l_plus")),
            ("sig", Some("signal")),
            ("x", None),
            ("", None),
        ] {
            assert_eq!(role_typed(typed), role, "{typed}");
        }
    }

    /// The nine alignments are three horizontal by three vertical positions, each read back as itself.
    #[test]
    fn the_nine_alignments_read_back_as_their_positions() {
        for (h, column) in ALIGNS.iter().enumerate() {
            for (v, word) in column.iter().enumerate() {
                assert_eq!(align_position(word), (h, v), "{word}");
            }
        }
        assert_eq!(ALIGNS[1][1], "centre");
        assert_eq!(align_position("a-newer-word"), (0, 0));
    }

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
