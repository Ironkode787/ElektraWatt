// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The symbol editor's screen (doc 07 §5): the main window's command bar with the drawing tools; on the left
//! the outline of what is edited, in the middle the drawing, on the right Properties, as in the main window. With
//! nothing in hand it shows a start card: the ways in. A part's device box is edited on the same screen
//! ([`device_box`]). Every edit dispatches the shared headless actions.

mod canvas;
mod device_box;
pub(crate) mod fields;
mod paint;
mod panels;

use crate::{
    app::Event,
    components::{self, Kind, Tip as _},
    icons::IconId,
    theme,
};
use egui::{Align, Layout, Pos2, Rect, Sense, Ui, UiBuilder, Vec2};
use ew_viewmodel::command_bar::{ButtonKind, CommandButton, CommandSize};
use ew_viewmodel::ew_api::symbols;
use ew_viewmodel::key;
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::shell::symbol_editor::{SymbolEditor, TOOL_KEYS};
use ew_viewmodel::strings;
use serde_json::{Value, json};

/// The text of `key` in the window's language.
fn label(key: strings::Key<'_>) -> &'static str {
    strings::text(key)
}

fn action(events: &mut Vec<Event>, name: &'static str, value: Value) {
    events.push(Event::Action(name, value));
}

/// An action that drops the draft: the model asks Save, Discard or Cancel first while it has unsaved changes, in the
/// house dialog.
fn ask(events: &mut Vec<Event>, name: &'static str, value: Value) {
    events.push(Event::SymbolAsk(name, value));
}

/// The start card's page.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum StartPage {
    /// The ways in.
    #[default]
    Ways,
    /// The library's symbols, to copy one.
    Copy,
    /// The project's parts that are drawn as a box, to edit one's device box, or a new part.
    Box,
}

/// What the window keeps of the editor between frames: its view of the drawing, a tool's half-made shape and its
/// options, and the start card's page.
#[derive(Default)]
pub(crate) struct EditorView {
    generation: Option<u64>,
    /// The model's tool last seen: a change drops what the tool before held half done.
    seen_tool: &'static str,
    zoom: f32,
    pan: Vec2,
    manual_view: bool,
    /// The drawing's size the view was last fitted at: the view fits again only at another size or on Fit.
    fitted: Option<Vec2>,
    /// Where each thing of the draft lies, by the draft's generation and revision.
    bounds: Option<((u64, u64), paint::Bounds)>,
    gesture: canvas::Gesture,
    box_gesture: device_box::BoxGesture,
    /// The first point of a shape of two points.
    start: Option<[f64; 2]>,
    /// The corners of a path so far.
    vertices: Vec<[f64; 2]>,
    /// Where the text being typed goes, and the text.
    text_at: Option<[f64; 2]>,
    text: String,
    /// The Connection point tool's designation and direction, the Text field tool's kind and the Link point tool's
    /// name.
    designation: String,
    direction: usize,
    slot_kind: String,
    link_name: String,
    /// The whole document as text, under More.
    advanced: String,
    json_error: bool,
    start_page: StartPage,
    source_filter: String,
    /// The drawing's top right corner in the last frame: the toasts stand under it, on the drawing, never over
    /// Properties (FT-1714).
    pub(crate) toast_corner: Option<Pos2>,
    /// The outline as made for the draft's generation and revision, the language and the discipline: a frame or a
    /// pointer move draws it, a change of the draft makes it again.
    outline: Option<(
        OutlineKey,
        std::rc::Rc<ew_viewmodel::shell::symbol_editor::Outline>,
    )>,
    /// The start card's list on show under its filter ([`start_rows`]).
    start_rows: Option<(StartKey, std::rc::Rc<StartRows>)>,
    /// The symbol the Copy page's preview shows (the row the pointer is on, else the first) and its shape, read once
    /// per symbol (FT-1810).
    copy_preview: Option<(
        String,
        Option<std::rc::Rc<ew_viewmodel::ew_api::output::SymbolShape>>,
    )>,
}

/// What the outline is made for: the draft's generation and revision, the language and the discipline.
type OutlineKey = (u64, u64, u64, &'static str);

/// What a start card's list is made for: the list (`copy`, `box`), the editor's generation, the list's length and
/// the filter typed; with the language.
type StartKey = (&'static str, u64, usize, String, u64);

/// A start card's rows: the index in its list, the row's words and its detail.
type StartRows = Vec<(usize, String, String)>;

/// The rows of a start card's list as `key` names them: made by `make` when the list, the filter or the language
/// changed since they were made, else those made then. A frame or a pointer move lays out the rows on show only, and
/// does not word every symbol or part of the libraries again.
fn start_rows(
    cache: &mut Option<(StartKey, std::rc::Rc<StartRows>)>,
    (list, generation, len, filter): (&'static str, u64, usize, String),
    make: impl FnOnce() -> StartRows,
) -> std::rc::Rc<StartRows> {
    let key = (list, generation, len, filter, strings::generation());
    match cache {
        Some((k, rows)) if *k == key => std::rc::Rc::clone(rows),
        _ => {
            let rows = std::rc::Rc::new(make());
            *cache = Some((key, std::rc::Rc::clone(&rows)));
            rows
        }
    }
}

/// The height of the editor's command row: the main window's.
pub(crate) fn toolbar_height(t: &theme::Tokens) -> f32 {
    t.size_command_row
}

/// The drawing tools of the bar: the action's word, the caption's key and the icon. Each has `symbol-tip-<word>`,
/// what it does and how.
const TOOLS: [(&str, strings::Key<'static>, &str); 13] = [
    ("select", key!("symbol-select"), "pointer"),
    ("line", key!("symbol-line"), "ew-shape-line"),
    ("rectangle", key!("symbol-rectangle"), "ew-shape-rectangle"),
    ("circle", key!("symbol-circle"), "ew-shape-circle"),
    ("ellipse", key!("symbol-ellipse"), "ew-shape-ellipse"),
    ("arc", key!("symbol-arc"), "ew-shape-arc"),
    ("polyline", key!("symbol-polyline"), "ew-shape-polyline"),
    ("polygon", key!("symbol-polygon"), "ew-shape-polygon"),
    ("text", key!("symbol-text"), "typography"),
    ("cp", key!("symbol-cp"), "plug-connected"),
    ("slot", key!("symbol-slot"), "text-size"),
    ("anchor", key!("symbol-anchor"), "current-location"),
    ("link", key!("symbol-link"), "link"),
];

/// A command of the editor's bar, with the main bar's sizing, painting and accessible name; `how` is one line on
/// what it does, under its name in the tooltip.
fn bar_command(
    ui: &mut Ui,
    key: strings::Key<'static>,
    icon: &'static str,
    size: CommandSize,
    [checked, enabled, menu]: [bool; 3],
    shortcut: String,
    how: Option<&str>,
) -> egui::Response {
    let name = label(key);
    crate::command_bar::inline_command(
        ui,
        &CommandButton {
            id: key.as_str(),
            label: how.map_or_else(|| name.to_owned(), |how| format!("{name}\n{how}")),
            bar_label: name.to_owned(),
            icon,
            size,
            shortcut,
            enabled,
            reason: (!enabled).then(|| label(key!("reason-no-project")).to_owned()),
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

/// Two small commands of the bar, one over the other, as the main bar stacks them.
fn stacked(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    let t = theme::current(ui.ctx());
    ui.allocate_ui_with_layout(
        Vec2::new(0.0, 2.0 * t.size_command_small + t.space_1),
        Layout::top_down(Align::Min),
        |ui| {
            ui.spacing_mut().item_spacing.y = t.space_1;
            add(ui);
        },
    );
}

/// The divider between two groups of the bar.
fn divider(ui: &mut Ui) {
    let t = theme::current(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(2.0 * t.space_2 + t.border_width, t.size_command_large_height),
        Sense::hover(),
    );
    ui.painter().line_segment(
        [rect.center_top(), rect.center_bottom()],
        theme::stroke(t.border_width, t.border_subtle),
    );
}

impl EditorView {
    fn sync(&mut self, shell: &ShellModel) {
        let Some(editor) = shell.symbol_editor() else {
            return;
        };
        if self.generation != Some(editor.generation) {
            *self = Self {
                generation: Some(editor.generation),
                zoom: 8.0,
                seen_tool: editor.tool,
                direction: 0,
                ..Default::default()
            };
        }
        if self.seen_tool != editor.tool {
            self.seen_tool = editor.tool;
            self.drop_gesture();
            // The Connection point tool starts at the next free number.
            if editor.tool == "cp" {
                let last = editor
                    .draft
                    .function
                    .cps
                    .iter()
                    .filter_map(|p| p.designation.as_str().parse::<u32>().ok())
                    .max();
                self.designation = last
                    .unwrap_or(editor.draft.symbol.cps.len() as u32)
                    .saturating_add(1)
                    .to_string();
            }
        }
    }

    fn tool_button(
        &mut self,
        ui: &mut Ui,
        shell: &ShellModel,
        editor: &SymbolEditor,
        word: &str,
        size: CommandSize,
        events: &mut Vec<Event>,
    ) -> bool {
        let Some((word, caption, icon)) = TOOLS.iter().find(|(w, ..)| *w == word).copied() else {
            return false;
        };
        // P8: the tooltip names the key of the keymap that chooses the tool.
        let key = TOOL_KEYS
            .iter()
            .find(|(tool, _)| *tool == word)
            .and_then(|(_, action)| ew_viewmodel::actions::find(action))
            .map(|m| shell.shortcut_text(m))
            .unwrap_or_default();
        // FT-1703: what the tool does and how, under its name.
        let how = label(strings::Key::run_time(&format!("symbol-tip-{word}")));
        let clicked = bar_command(
            ui,
            caption,
            icon,
            size,
            [editor.tool == word, true, false],
            key,
            Some(how),
        )
        .clicked();
        if clicked {
            action(events, "symbol.tool", json!({ "tool": word }));
        }
        clicked
    }

    /// The editor's command row: the tools, the grid, and at the right end Test on a page, Save and Close editor.
    pub(crate) fn toolbar(&mut self, ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
        self.sync(shell);
        let Some(editor) = shell.symbol_editor() else {
            return;
        };
        let t = theme::current(ui.ctx());
        ui.spacing_mut().item_spacing = Vec2::new(t.space_1, t.space_1);
        if let Some(b) = editor.device_box() {
            self.box_toolbar(ui, shell, b, events);
            return;
        }
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            if !editor.start {
                // The ways in: another symbol, a copy, a device box, an import.
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
                    action(events, "symbol.start", json!({}));
                }
                divider(ui);
                self.tool_button(ui, shell, editor, "select", CommandSize::Large, events);
                stacked(ui, |ui| {
                    self.tool_button(ui, shell, editor, "line", CommandSize::Small, events);
                    self.tool_button(ui, shell, editor, "rectangle", CommandSize::Small, events);
                });
                stacked(ui, |ui| {
                    self.tool_button(ui, shell, editor, "circle", CommandSize::Small, events);
                    let more = ["ellipse", "arc", "polyline", "polygon"];
                    let menu = bar_command(
                        ui,
                        key!("symbol-more-shapes"),
                        "category",
                        CommandSize::Small,
                        [more.contains(&editor.tool), true, true],
                        String::new(),
                        None,
                    );
                    egui::Popup::menu(&menu).show(|ui| {
                        for word in more {
                            if self.tool_button(ui, shell, editor, word, CommandSize::Small, events) {
                                ui.close();
                            }
                        }
                    });
                });
                stacked(ui, |ui| {
                    self.tool_button(ui, shell, editor, "text", CommandSize::Small, events);
                });
                divider(ui);
                self.tool_button(ui, shell, editor, "cp", CommandSize::Large, events);
                stacked(ui, |ui| {
                    self.tool_button(ui, shell, editor, "slot", CommandSize::Small, events);
                    // The marks a rare symbol needs: its cross-reference point, a mechanical link point.
                    let more = ["anchor", "link"];
                    let menu = bar_command(
                        ui,
                        key!("symbol-more-marks"),
                        "dots",
                        CommandSize::Small,
                        [more.contains(&editor.tool), true, true],
                        String::new(),
                        None,
                    );
                    egui::Popup::menu(&menu).show(|ui| {
                        for word in more {
                            if self.tool_button(ui, shell, editor, word, CommandSize::Small, events) {
                                ui.close();
                            }
                        }
                    });
                });
                divider(ui);
                grid_controls(ui, editor, events);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                // Close editor is the way back to the project: the editor's one primary button, in the corner the eye
                // goes to for it. It keeps the draft and its history.
                if components::button(ui, Kind::Primary, label(key!("symbol-close")), true, 0.0).clicked() {
                    action(events, "symbol.close", json!({}));
                }
                if editor.start {
                    return;
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
                    Some(label(key!("symbol-save-how"))),
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
                            [false, shell.has_project(), false],
                            String::new(),
                            Some(label(key!("symbol-scratch-how"))),
                        )
                        .clicked()
                            && shell.has_project()
                        {
                            action(events, "symbol.scratch", json!({}));
                        }
                        let menu = bar_command(
                            ui,
                            key!("symbol-draft-menu"),
                            "dots",
                            CommandSize::Small,
                            [false, true, true],
                            String::new(),
                            None,
                        );
                        egui::Popup::menu(&menu).show(|ui| {
                            // FT-1766: the way from a symbol to a device: a part whose main function the saved
                            // symbol draws.
                            if ui
                                .add_enabled(
                                    shell.has_project(),
                                    egui::Button::new(label(key!("symbol-make-part"))),
                                )
                                .on_hover_text(label(key!("symbol-make-part-how")))
                                .clicked()
                            {
                                let symbol = editor.published().unwrap_or_default();
                                action(events, "part.new", json!({ "symbol": symbol }));
                                ui.close();
                            }
                            if ui.button(label(key!("symbol-export"))).clicked() {
                                action(events, "symbol.export", json!({}));
                                ui.close();
                            }
                            // Discard draft is the explicit way to drop the draft; Close editor keeps it.
                            if ui.button(label(key!("symbol-discard-draft"))).clicked() {
                                ask(events, "symbol.close", json!({"discard": true}));
                                ui.close();
                            }
                        });
                    });
                });
            });
        });
    }

    /// The editor in the page's place: the outline, the drawing and Properties, or the start card.
    pub fn show(&mut self, ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
        self.sync(shell);
        let Some(editor) = shell.symbol_editor() else {
            return;
        };
        let t = theme::current(ui.ctx());
        let full = ui.max_rect();
        ui.allocate_rect(full, Sense::hover());
        // F1 on a part of the screen (the start card, the outline, the drawing, Properties, the device box): its
        // section of the editor's page.
        let help = |ui: &Ui, rect: Rect, part: ew_viewmodel::help::EditorPart| {
            crate::help::offer(ui, rect, || {
                let topic = ew_viewmodel::help::symbol_editor_part(editor, part);
                (topic.to_owned(), strings::text(key!("symbol-title")).to_owned())
            });
        };
        help(ui, full, ew_viewmodel::help::EditorPart::Other);
        if editor.start && editor.device_box().is_none() {
            self.toast_corner = Some(full.right_top());
            self.start_card(ui, full, shell, editor, events);
            return;
        }
        let styles = shell.symbol_text_styles();
        // The side panels keep their width; the drawing takes the room (doc 16a §2).
        let left_w = (t.size_panel_left + 2.0 * t.space_4).min(full.width() * 0.3);
        let right_w = (t.size_panel_left + t.size_property_label).min(full.width() * 0.3);
        let left = Rect::from_min_max(full.min, Pos2::new(full.left() + left_w, full.bottom()));
        let right = Rect::from_min_max(Pos2::new(full.right() - right_w, full.top()), full.max);
        let centre = Rect::from_min_max(
            Pos2::new(left.right() + t.border_width, full.top()),
            Pos2::new(right.left() - t.border_width, full.bottom()),
        );
        for panel in [left, right] {
            ui.painter().rect_filled(panel, 0.0, t.surface_panel.color32());
        }
        help(ui, centre, ew_viewmodel::help::EditorPart::Drawing);
        help(ui, left, ew_viewmodel::help::EditorPart::Outline);
        for x in [left.right(), right.left()] {
            ui.painter().line_segment(
                [Pos2::new(x, full.top()), Pos2::new(x, full.bottom())],
                theme::stroke(t.border_width, t.border_default),
            );
        }
        // A part's device box takes the screen while it is edited.
        if let Some(b) = editor.device_box() {
            ui.scope_builder(UiBuilder::new().max_rect(left), |ui| {
                ui.set_clip_rect(left.intersect(ui.clip_rect()));
                self.box_outline(ui, b, events);
            });
            ui.scope_builder(UiBuilder::new().max_rect(right), |ui| {
                ui.set_clip_rect(right.intersect(ui.clip_rect()));
                ui.push_id(("box", b.draft.copy), |ui| self.box_properties(ui, b, events));
            });
            ui.scope_builder(UiBuilder::new().max_rect(centre), |ui| {
                ui.set_clip_rect(centre.intersect(ui.clip_rect()));
                self.box_drawing(ui, centre, b, events);
            });
            if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
                && !b.selection.is_empty()
            {
                action(events, "part.box_select", json!({ "pins": [] }));
            }
            return;
        }
        ui.scope_builder(UiBuilder::new().max_rect(left), |ui| {
            ui.set_clip_rect(left.intersect(ui.clip_rect()));
            self.outline(ui, editor, events);
        });
        ui.scope_builder(UiBuilder::new().max_rect(right), |ui| {
            ui.set_clip_rect(right.intersect(ui.clip_rect()));
            ui.push_id(editor.generation, |ui| self.properties(ui, shell, editor, events));
        });
        ui.scope_builder(UiBuilder::new().max_rect(centre), |ui| {
            ui.set_clip_rect(centre.intersect(ui.clip_rect()));
            self.drawing(ui, centre, shell, editor, &styles, events);
        });
        // Escape ends what a tool holds half done; at rest it returns to Select, and with Select it clears the
        // selection, as on a page.
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            let busy = self.start.is_some()
                || self.text_at.is_some()
                || !self.vertices.is_empty()
                || self.gesture != canvas::Gesture::None;
            if busy {
                self.drop_gesture();
            } else if editor.tool != "select" {
                action(events, "symbol.tool", json!({ "tool": "select" }));
            } else if !editor.selection.is_empty() {
                action(events, "symbol.select", json!({ "items": [] }));
            }
        }
    }

    /// The middle column: why the last action did nothing, the drawing with the tool's options on it, and under it
    /// the symbol turned and mirrored.
    fn drawing(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        shell: &ShellModel,
        editor: &SymbolEditor,
        styles: &symbols::TextStyleTable,
        events: &mut Vec<Event>,
    ) {
        let t = theme::current(ui.ctx());
        let mut top = rect.top();
        // Why the last action did nothing, above the drawing: a refused Save or Test must answer where the eye is.
        if let Some(error) = &editor.error {
            let banner = ui.scope_builder(
                UiBuilder::new().max_rect(Rect::from_min_max(
                    rect.min,
                    Pos2::new(rect.right(), rect.bottom()),
                )),
                |ui| {
                    egui::Frame::NONE
                        .fill(t.error_subtle.color32())
                        .inner_margin(egui::Margin::symmetric(
                            theme::margin_px(t.space_4),
                            theme::margin_px(t.space_3),
                        ))
                        .show(ui, |ui| {
                            ui.set_width(rect.width() - 2.0 * t.space_4);
                            ui.colored_label(t.error.color32(), error);
                            // A page on another connection grid offers a new page with the symbol's grid: the
                            // published symbol placed there, or a test copy.
                            let (text, offer) = if editor.published().is_some() {
                                (key!("symbol-place-new-page"), "symbol.new_page")
                            } else {
                                (key!("symbol-scratch"), "symbol.scratch")
                            };
                            if editor.offer_new_page
                                && components::button(
                                    ui,
                                    Kind::Secondary,
                                    label(text),
                                    shell.has_project(),
                                    0.0,
                                )
                                .clicked()
                            {
                                action(events, offer, json!({}));
                            }
                        });
                },
            );
            top = banner.response.rect.bottom();
        }
        self.toast_corner = Some(Pos2::new(rect.right(), top));
        // The symbol turned and mirrored, under the drawing.
        let strip_h = 3.0 * t.size_row;
        let caption_h = t.size_row;
        let strip = Rect::from_min_max(
            Pos2::new(rect.left(), rect.bottom() - strip_h - caption_h - t.space_3),
            rect.max,
        );
        let canvas = Rect::from_min_max(
            Pos2::new(rect.left(), top),
            Pos2::new(rect.right(), strip.top() - t.border_width),
        );
        self.canvas(ui, canvas, editor, styles, events);
        self.tool_card(ui, canvas, editor);
        // Fit drawing, in the drawing's corner.
        let fit = Rect::from_min_size(
            Pos2::new(
                canvas.right() - t.space_3 - t.size_control,
                canvas.top() + t.space_3,
            ),
            Vec2::splat(t.size_control),
        );
        ui.scope_builder(UiBuilder::new().max_rect(fit), |ui| {
            if components::icon_button(ui, IconId::maximize, label(key!("symbol-fit"))).clicked() {
                self.manual_view = false;
                self.fitted = None;
            }
        });
        ui.painter().line_segment(
            [strip.left_top(), strip.right_top()],
            theme::stroke(t.border_width, t.border_subtle),
        );
        ui.painter().rect_filled(strip, 0.0, t.surface_panel.color32());
        ui.scope_builder(
            UiBuilder::new().max_rect(strip.shrink2(Vec2::new(t.space_4, t.space_2))),
            |ui| self.variants(ui, editor, styles, events),
        );
    }

    /// The symbol turned and mirrored: its eight orientations, each a click to see it large.
    fn variants(
        &mut self,
        ui: &mut Ui,
        editor: &SymbolEditor,
        styles: &symbols::TextStyleTable,
        events: &mut Vec<Event>,
    ) {
        let t = theme::current(ui.ctx());
        let draft = &editor.draft;
        let symbol_name = ew_viewmodel::shell::symbol_editor::display_name(draft);
        ui.label(
            egui::RichText::new(label(key!("symbol-variants")))
                .font(theme::font(&t.type_small))
                .color(t.text_secondary.color32()),
        );
        let pins: Vec<String> = draft
            .symbol
            .cps
            .iter()
            .map(|cp| {
                draft
                    .function
                    .cps
                    .get(&cp.fn_cp)
                    .map_or_else(|| cp.idx.to_string(), |p| p.designation.to_string())
            })
            .collect();
        let height = (ui.available_height() - t.space_1).max(t.size_row);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = t.space_2;
            // The eight cells share the strip's width, so a wider window shows them larger.
            let width = ((ui.available_width() - 7.0 * t.space_2) / 8.0).max(t.size_control);
            for variant in 0..8u8 {
                let (r, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());
                // A preview is named by the symbol, its variant and its pins (doc 07 §5).
                let letter = char::from(b'A' + variant).to_string();
                let name = if pins.is_empty() {
                    strings::format(key!("symbol-variant-name-plain"), &[&symbol_name, &letter])
                } else {
                    strings::format(
                        key!("symbol-variant-name"),
                        &[&symbol_name, &letter, &pins.join(", ")],
                    )
                };
                let chosen = variant == editor.variant;
                response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, chosen, &name));
                let painter = ui.painter_at(r);
                painter.rect_filled(
                    r,
                    t.radius_sm,
                    if chosen {
                        t.canvas_selection.alpha(24).over(t.canvas_sheet).color32()
                    } else {
                        t.canvas_sheet.color32()
                    },
                );
                painter.rect_stroke(
                    r,
                    t.radius_sm,
                    theme::stroke(t.border_width, if chosen { t.accent } else { t.border_subtle }),
                    egui::StrokeKind::Inside,
                );
                let letter_at = r.left_top() + Vec2::new(t.space_2, t.space_1);
                let letter_rect = painter.text(
                    letter_at,
                    egui::Align2::LEFT_TOP,
                    &letter,
                    theme::font(&t.type_small),
                    t.text_secondary.color32(),
                );
                // The drawing, its texts included, fits inside its cell, right of its letter.
                let area = Rect::from_min_max(
                    Pos2::new(letter_rect.right() + t.space_2, r.top() + t.space_2),
                    r.max - Vec2::splat(t.space_2),
                );
                if let Some(b) = paint::drawn_bounds(draft, variant, styles) {
                    let (origin, scale) = paint::variant_view(b, area);
                    paint::paint(&painter, draft, variant, origin, scale, false, styles);
                }
                if response.clicked() {
                    action(events, "symbol.variant", json!({ "variant": variant }));
                }
            }
        });
    }

    /// The running tool's options and what it asks for, on a card at the drawing's top left (as a page's tool
    /// options): shown while a drawing tool runs.
    fn tool_card(&mut self, ui: &mut Ui, canvas: Rect, editor: &SymbolEditor) {
        let t = theme::current(ui.ctx());
        if editor.tool == "select" || editor.variant != 0 {
            return;
        }
        let at = canvas.left_top() + Vec2::splat(t.space_3);
        egui::Area::new(egui::Id::unique("symbol-tool-card"))
            .fixed_pos(at)
            .order(egui::Order::Middle)
            .constrain_to(canvas)
            .show(ui.ctx(), |ui| {
                components::card_frame(&t).show(ui, |ui| {
                    ui.set_max_width(canvas.width() - 2.0 * t.space_3);
                    ui.horizontal(|ui| {
                        let field = |ui: &mut Ui, key: strings::Key<'_>, value: &mut String, width: f32| {
                            let caption = ui.label(label(key));
                            ui.add(egui::TextEdit::singleline(value).desired_width(width))
                                .labelled_by(caption.id);
                        };
                        match editor.tool {
                            "cp" => {
                                field(
                                    ui,
                                    key!("symbol-designation"),
                                    &mut self.designation,
                                    3.0 * t.size_row,
                                );
                                // The first follows the lead the point ends (a point always faced left).
                                let words = [
                                    key!("symbol-direction-lead"),
                                    key!("symbol-up"),
                                    key!("symbol-right"),
                                    key!("symbol-down"),
                                    key!("symbol-left"),
                                ];
                                let caption = ui.label(label(key!("symbol-direction")));
                                crate::components::choice(
                                    ui,
                                    egui::ComboBox::from_id_salt("symbol-cp-direction")
                                        .selected_text(label(words[self.direction.min(4)])),
                                    |ui| {
                                        for (i, k) in words.iter().enumerate() {
                                            ui.selectable_value(&mut self.direction, i, label(*k));
                                        }
                                    },
                                )
                                .response
                                .labelled_by(caption.id);
                            }
                            "slot" => {
                                let kinds = editor.slot_kinds();
                                if self.slot_kind.is_empty() {
                                    self.slot_kind = "tag".to_owned();
                                }
                                let shown = kinds
                                    .iter()
                                    .find(|(w, _)| *w == self.slot_kind)
                                    .map_or_else(String::new, |(_, text)| text.clone());
                                let caption = ui.label(label(key!("symbol-slot-kind")));
                                crate::components::choice(
                                    ui,
                                    egui::ComboBox::from_id_salt("symbol-new-slot").selected_text(shown),
                                    |ui| {
                                        for (word, text) in kinds {
                                            ui.selectable_value(&mut self.slot_kind, word, text);
                                        }
                                    },
                                )
                                .response
                                .labelled_by(caption.id);
                            }
                            "link" => field(ui, key!("symbol-name"), &mut self.link_name, 5.0 * t.size_row),
                            _ => {}
                        }
                        ui.label(
                            egui::RichText::new(label(strings::Key::run_time(&format!(
                                "symbol-tip-{}",
                                editor.tool
                            ))))
                            .font(theme::font(&t.type_small))
                            .color(t.text_secondary.color32()),
                        );
                    });
                });
            });
    }

    /// The left panel: what the symbol consists of, each row a click to the thing, and what is left to do.
    fn outline(&mut self, ui: &mut Ui, editor: &SymbolEditor, events: &mut Vec<Event>) {
        let t = theme::current(ui.ctx());
        let key = (
            editor.generation,
            editor.revision,
            strings::generation(),
            editor.discipline(),
        );
        let outline = match &self.outline {
            Some((k, o)) if *k == key => std::rc::Rc::clone(o),
            _ => {
                let o = std::rc::Rc::new(editor.outline());
                self.outline = Some((key, std::rc::Rc::clone(&o)));
                o
            }
        };
        panels::panel_header(ui, label(key!("symbol-outline")));
        panels::body(ui, "symbol-outline", |ui| {
            panels::title_lines(ui, &outline.title, &outline.what);
            let mods = ui.input(|i| i.modifiers);
            let mode = if mods.shift {
                "add"
            } else if mods.command || mods.ctrl {
                "toggle"
            } else {
                "replace"
            };
            // An import with more drawings says so first, with the drawings to choose from.
            if !outline.candidates.is_empty() {
                let id = ui.make_persistent_id("symbol-outline-candidates");
                let title = label(key!("symbol-candidates"));
                if components::section_header(ui, id, title, Some(outline.candidates.len()), true) {
                    for (i, name) in outline.candidates.iter().enumerate() {
                        if panels::list_row(ui, name, "", false).clicked() {
                            ask(events, "symbol.candidate", json!({ "index": i }));
                        }
                    }
                }
            }
            for group in &outline.groups {
                let id = ui.make_persistent_id(("symbol-outline", group.key));
                if !components::section_header(ui, id, &group.label, Some(group.rows.len()), true) {
                    continue;
                }
                for row in &group.rows {
                    let selected = editor.selection.contains(&row.item);
                    if panels::list_row(ui, &row.label, &row.detail, selected).clicked() {
                        action(
                            events,
                            "symbol.select",
                            json!({ "items": [row.item.word()], "mode": mode }),
                        );
                    }
                }
                if group.rows.is_empty() {
                    ui.horizontal(|ui| {
                        ui.add_space(t.size_tree_indent + t.space_2);
                        ui.label(
                            egui::RichText::new(label(strings::Key::run_time(&format!(
                                "symbol-outline-empty-{}",
                                group.key
                            ))))
                            .font(theme::font(&t.type_small))
                            .color(t.text_tertiary.color32()),
                        );
                    });
                }
            }
            if !outline.import_notes.is_empty() {
                let id = ui.make_persistent_id("symbol-outline-import");
                let title = label(key!("symbol-import-notes"));
                if components::section_header(ui, id, title, Some(outline.import_notes.len()), false) {
                    for note in &outline.import_notes {
                        ui.add(
                            egui::Label::new(egui::RichText::new(note).font(theme::font(&t.type_small)))
                                .wrap(),
                        );
                    }
                    let mut reviewed = outline.import_reviewed;
                    if ui
                        .checkbox(&mut reviewed, label(key!("symbol-reviewed")))
                        .changed()
                    {
                        action(
                            events,
                            "symbol.prop",
                            json!({ "key": "reviewed", "value": reviewed }),
                        );
                    }
                }
            }
            // What keeps the symbol from being saved, each a click to the thing; none: ready to save.
            let id = ui.make_persistent_id("symbol-outline-todo");
            ui.add_space(t.space_2);
            if outline.todo.is_empty() {
                panels::ready_line(ui, label(key!("symbol-valid")));
            } else if components::section_header(
                ui,
                id,
                label(key!("symbol-todo")),
                Some(outline.todo.len()),
                true,
            ) {
                for todo in &outline.todo {
                    if panels::todo_row(ui, &todo.text).clicked() {
                        let items: Vec<String> = todo.item.iter().map(|i| i.word()).collect();
                        action(events, "symbol.select", json!({ "items": items }));
                    }
                }
            }
        });
    }

    /// The right panel: the rows of what is selected, or of the symbol itself.
    fn properties(
        &mut self,
        ui: &mut Ui,
        shell: &ShellModel,
        editor: &SymbolEditor,
        events: &mut Vec<Event>,
    ) {
        let t = theme::current(ui.ctx());
        let Some(sheet) = shell.symbol_sheet() else {
            return;
        };
        panels::panel_header(ui, label(key!("symbol-properties")));
        panels::body(ui, "symbol-properties", |ui| {
            panels::title_lines(ui, &sheet.title, &sheet.line);
            // The rows belong to the selection: another selection shows its own, never a field's leftover typing.
            let salt: Vec<String> = editor.selection.iter().map(|i| i.word()).collect();
            let symbol = editor.selection.is_empty();
            ui.push_id(salt, |ui| {
                panels::sheet_rows(
                    ui,
                    &sheet.rows,
                    &sheet.title,
                    "symbol.prop",
                    events,
                    |ui, events| {
                        // The folder picker, under the Library folder row.
                        if symbol && panels::button(ui, key!("symbol-choose-folder")) {
                            action(events, "symbol.directory", json!({}));
                        }
                    },
                );
            });
            ui.add_space(t.space_3);
            if symbol {
                self.whole_document(ui, editor, events);
                return;
            }
            // What is done with the selection, beside its keys.
            ui.horizontal_wrapped(|ui| {
                if editor.selection.len() > 1 {
                    let line_up =
                        components::button(ui, Kind::Secondary, label(key!("symbol-line-up")), true, 0.0);
                    egui::Popup::menu(&line_up).show(|ui| {
                        for to in ["left", "centre", "right", "top", "middle", "bottom"] {
                            let words = label(strings::Key::run_time(&format!("symbol-line-up-{to}")));
                            if ui.button(words).clicked() {
                                action(events, "symbol.align", json!({ "to": to }));
                                ui.close();
                            }
                        }
                    });
                }
                if panels::button(ui, key!("symbol-duplicate")) {
                    action(events, "symbol.duplicate", json!({}));
                }
                if panels::button(ui, key!("symbol-delete")) {
                    action(events, "symbol.delete", json!({}));
                }
            });
        });
    }

    /// The whole draft as its document, for what the rows do not show (other text arrangements, an import's extra
    /// members): an expert's way, closed until opened.
    fn whole_document(&mut self, ui: &mut Ui, editor: &SymbolEditor, events: &mut Vec<Event>) {
        let id = ui.make_persistent_id("symbol-whole-document");
        if !components::section_header(ui, id, label(key!("symbol-advanced")), None, false) {
            self.advanced.clear();
            return;
        }
        if self.advanced.is_empty() {
            self.advanced = serde_json::to_string_pretty(&editor.draft).unwrap_or_default();
        }
        let advanced = ui.add(
            egui::TextEdit::multiline(&mut self.advanced)
                .code_editor()
                .desired_rows(12)
                .desired_width(ui.available_width()),
        );
        let name = label(key!("symbol-advanced"));
        ui.ctx()
            .accesskit_node_builder(advanced.id, |n| n.set_label(name));
        ui.horizontal_wrapped(|ui| {
            if panels::button(ui, key!("symbol-refresh")) {
                self.advanced = serde_json::to_string_pretty(&editor.draft).unwrap_or_default();
                self.json_error = false;
            }
            if panels::button(ui, key!("symbol-apply")) {
                match serde_json::from_str::<Value>(&self.advanced) {
                    Ok(value) => {
                        action(events, "symbol.replace", json!({ "document": value }));
                        self.json_error = false;
                    }
                    Err(_) => self.json_error = true,
                }
            }
        });
        if self.json_error {
            ui.colored_label(
                theme::current(ui.ctx()).error.color32(),
                label(key!("symbol-json-invalid")),
            );
        }
    }

    /// The start card: with nothing in hand the editor shows the ways in, not an empty drawing with thirteen
    /// tools.
    fn start_card(
        &mut self,
        ui: &mut Ui,
        full: Rect,
        shell: &ShellModel,
        editor: &SymbolEditor,
        events: &mut Vec<Event>,
    ) {
        let t = theme::current(ui.ctx());
        // A third of a wide window: the lists of parts and symbols use the room of 2560 points.
        let width = crate::dialog::grown(t.size_dialog_form, 1.0 / 3.0, full.width())
            .min(full.width() - 2.0 * t.space_6);
        let card = Rect::from_min_size(
            Pos2::new(full.center().x - width / 2.0, full.top() + t.space_7),
            Vec2::new(width, full.height() - 2.0 * t.space_7),
        );
        ui.scope_builder(UiBuilder::new().max_rect(card), |ui| {
            components::card_frame(&t).show(ui, |ui| {
                ui.set_width(width - 2.0 * t.space_4);
                ui.spacing_mut().item_spacing.y = t.space_2;
                match self.start_page {
                    StartPage::Ways => self.start_ways(ui, shell, editor, events),
                    StartPage::Copy => self.start_copy(ui, shell, editor, events),
                    StartPage::Box => self.start_box(ui, shell, events),
                }
            });
        });
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            if self.start_page == StartPage::Ways {
                action(events, "symbol.start", json!({ "leave": true }));
            } else {
                self.start_page = StartPage::Ways;
            }
        }
    }

    fn start_ways(
        &mut self,
        ui: &mut Ui,
        shell: &ShellModel,
        editor: &SymbolEditor,
        events: &mut Vec<Event>,
    ) {
        let t = theme::current(ui.ctx());
        ui.label(
            egui::RichText::new(label(key!("symbol-start-title")))
                .font(theme::font(&t.type_title))
                .color(t.text_primary.color32()),
        );
        ui.label(
            egui::RichText::new(label(key!("symbol-start-line")))
                .font(theme::font(&t.type_body))
                .color(t.text_secondary.color32()),
        );
        ui.add_space(t.space_3);
        if way(
            ui,
            IconId::file_plus,
            label(key!("symbol-new-symbol")),
            label(key!("symbol-start-new-line")),
        )
        .clicked()
        {
            ask(events, "symbol.new", json!({ "discard": true }));
        }
        if way(
            ui,
            IconId::files,
            label(key!("symbol-start-copy")),
            label(key!("symbol-start-copy-line")),
        )
        .clicked()
        {
            self.start_page = StartPage::Copy;
            self.source_filter.clear();
        }
        if way(
            ui,
            IconId::package,
            label(key!("symbol-start-box")),
            label(key!("symbol-start-box-line")),
        )
        .clicked()
        {
            self.start_page = StartPage::Box;
            self.source_filter.clear();
        }
        if way(
            ui,
            IconId::file_import,
            label(key!("symbol-start-import")),
            label(key!("symbol-start-import-line")),
        )
        .clicked()
        {
            ask(events, "symbol.open", json!({}));
        }
        // The draft in hand after the ways in, which keep their places (FT-1810: on top it moved every entry down
        // after a save); the editor keeps it until it is saved or discarded.
        if editor.in_hand() {
            let name = strings::format(
                key!("symbol-start-continue"),
                &[&ew_viewmodel::shell::symbol_editor::display_name(&editor.draft)],
            );
            if way(ui, IconId::pencil, &name, label(editor.state_key())).clicked() {
                action(events, "symbol.start", json!({ "leave": true }));
            }
        }
        let recent = shell.symbol_recents();
        if !recent.is_empty() {
            ui.add_space(t.space_3);
            ui.label(
                egui::RichText::new(label(key!("symbol-start-recent")))
                    .font(theme::font(&theme::TypeToken {
                        semibold: true,
                        ..t.type_small
                    }))
                    .color(t.text_secondary.color32()),
            );
            for (address, name, library) in recent {
                if panels::list_row(ui, &name, &library, false).clicked() {
                    ask(events, "symbol.edit", json!({ "symbol": address }));
                }
            }
        }
        if let Some(error) = &editor.error {
            ui.add_space(t.space_2);
            ui.colored_label(t.error.color32(), error);
        }
    }

    /// The project's parts that are drawn as a box with pins of their own, to edit one's device box, and a new part.
    fn start_box(&mut self, ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
        let t = theme::current(ui.ctx());
        ui.horizontal(|ui| {
            if components::icon_button(ui, IconId::arrow_left, label(key!("symbol-start-back"))).clicked() {
                self.start_page = StartPage::Ways;
            }
            ui.label(
                egui::RichText::new(label(key!("symbol-start-box")))
                    .font(theme::font(&t.type_title))
                    .color(t.text_primary.color32()),
            );
        });
        ui.label(
            egui::RichText::new(label(key!("symbol-start-box-line")))
                .font(theme::font(&t.type_body))
                .color(t.text_secondary.color32()),
        );
        if !shell.has_project() {
            ui.label(
                egui::RichText::new(label(key!("symbol-start-box-no-project")))
                    .color(t.text_secondary.color32()),
            );
            return;
        }
        if way(
            ui,
            IconId::file_plus,
            label(key!("symbol-start-box-new")),
            label(key!("symbol-start-box-new-line")),
        )
        .clicked()
        {
            action(events, "part.box", json!({ "new": true }));
        }
        ui.add_space(t.space_2);
        // Typed at once, as the symbols' list is.
        let filter = components::filter_field(
            ui,
            &mut self.source_filter,
            label(key!("symbol-start-box-filter")),
        );
        if !filter.has_focus() && self.source_filter.is_empty() && ui.memory(|m| m.focused().is_none()) {
            filter.request_focus();
        }
        let wanted = self.source_filter.to_lowercase();
        let parts = shell.box_parts();
        let generation = shell.symbol_editor().map_or(0, |e| e.generation);
        let key = ("box", generation, parts.len(), wanted.clone());
        let rows = start_rows(&mut self.start_rows, key, || {
            parts
                .iter()
                .enumerate()
                .filter(|(_, (_, number, detail))| {
                    number.to_lowercase().contains(&wanted) || detail.to_lowercase().contains(&wanted)
                })
                .map(|(i, (_, number, detail))| (i, number.clone(), detail.clone()))
                .collect()
        });
        // Why a part is missing, above the list, which takes the card's height.
        let note = if parts.is_empty() {
            key!("symbol-start-box-none")
        } else if rows.is_empty() {
            key!("symbol-start-box-no-match")
        } else {
            key!("symbol-start-box-elsewhere")
        };
        ui.add(
            egui::Label::new(
                egui::RichText::new(label(note))
                    .font(theme::font(&t.type_small))
                    .color(t.text_secondary.color32()),
            )
            .wrap(),
        );
        ui.spacing_mut().scroll.floating = false;
        egui::ScrollArea::vertical()
            .id_salt("symbol-start-parts")
            .auto_shrink([false, false])
            .show_rows(ui, t.size_row, rows.len(), |ui, range| {
                for (i, number, detail) in &rows[range] {
                    if panels::list_row(ui, number, detail, false).clicked()
                        && let Some((part, _, _)) = parts.get(*i)
                    {
                        action(events, "part.box", json!({ "part": part }));
                    }
                }
            });
    }

    /// The library's symbols, to copy one: the original stays as it is.
    fn start_copy(
        &mut self,
        ui: &mut Ui,
        shell: &ShellModel,
        editor: &SymbolEditor,
        events: &mut Vec<Event>,
    ) {
        let t = theme::current(ui.ctx());
        ui.horizontal(|ui| {
            if components::icon_button(ui, IconId::arrow_left, label(key!("symbol-start-back"))).clicked() {
                self.start_page = StartPage::Ways;
            }
            ui.label(
                egui::RichText::new(label(key!("symbol-start-copy")))
                    .font(theme::font(&t.type_title))
                    .color(t.text_primary.color32()),
            );
        });
        ui.label(
            egui::RichText::new(label(key!("symbol-start-copy-line")))
                .font(theme::font(&t.type_body))
                .color(t.text_secondary.color32()),
        );
        let filter =
            components::filter_field(ui, &mut self.source_filter, label(key!("symbol-source-filter")));
        if !filter.has_focus() && self.source_filter.is_empty() && ui.memory(|m| m.focused().is_none()) {
            filter.request_focus();
        }
        let wanted = self.source_filter.to_lowercase();
        let key = ("copy", editor.generation, editor.sources.len(), wanted.clone());
        let rows = start_rows(&mut self.start_rows, key, || {
            editor
                .sources
                .iter()
                .enumerate()
                .map(|(i, s)| (i, s, s.words()))
                .filter(|(_, s, (name, _))| {
                    name.to_lowercase().contains(&wanted)
                        || s.key
                            .as_deref()
                            .is_some_and(|k| k.to_lowercase().contains(&wanted))
                })
                // In the browser's order (the editor sorts its sources so), each row naming what it is filed under.
                .map(|(i, s, (name, library))| {
                    let filed = ew_viewmodel::tools::category_words(&s.category)
                        .unwrap_or_else(|| label(key!("category-other")));
                    (i, name, format!("{filed} \u{b7} {library}"))
                })
                .collect()
        });
        ui.spacing_mut().scroll.floating = false;
        // The list at the left, the symbol the pointer is on at the right, as the symbol browser shows it.
        let mut hovered: Option<usize> = None;
        let preview_w = (ui.available_width() * 0.4).max(t.size_dialog_form / 3.0);
        ui.horizontal_top(|ui| {
            let list_w = ui.available_width() - preview_w - t.space_3;
            ui.allocate_ui(Vec2::new(list_w, ui.available_height()), |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("symbol-start-sources")
                    .auto_shrink([false, false])
                    .show_rows(ui, t.size_row, rows.len(), |ui, range| {
                        for (i, name, detail) in &rows[range] {
                            let row = panels::list_row(ui, name, detail, false);
                            if row.hovered() {
                                hovered = Some(*i);
                            }
                            if row.clicked()
                                && let Some(source) = editor.sources.get(*i)
                            {
                                ask(events, "symbol.copy_edit", json!({ "symbol": source.source }));
                            }
                        }
                    });
            });
            let shown = hovered.or_else(|| rows.first().map(|r| r.0));
            if let Some(source) = shown.and_then(|i| editor.sources.get(i)) {
                if self
                    .copy_preview
                    .as_ref()
                    .is_none_or(|(s, _)| *s != source.source)
                {
                    let shape = shell.symbol_source_shape(source).map(std::rc::Rc::new);
                    self.copy_preview = Some((source.source.clone(), shape));
                }
                let side = preview_w.min(ui.available_height());
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(side), Sense::hover());
                let shape = self.copy_preview.as_ref().and_then(|(_, s)| s.as_deref());
                crate::picker::picture(ui, &t, rect, shape);
            }
        });
        if rows.is_empty() {
            ui.label(
                egui::RichText::new(label(key!("symbol-start-no-match"))).color(t.text_secondary.color32()),
            );
        }
    }
}

/// One way in on the start card: an icon, what it starts in `type.body.strong` and a sentence under it; a button the
/// whole row wide.
fn way(ui: &mut Ui, icon: IconId, title: &str, sentence: &str) -> egui::Response {
    let t = theme::current(ui.ctx());
    let height = 2.0 * t.size_row;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, title));
    ui.painter().rect_stroke(
        rect,
        theme::radius(t.radius_md),
        theme::stroke(t.border_width, t.border_subtle),
        egui::StrokeKind::Inside,
    );
    crate::command_bar::state_overlay(ui, &t, &response, true, t.radius_md);
    components::focus_ring(ui, &response, t.radius_md);
    let icon_rect = Rect::from_center_size(
        Pos2::new(rect.left() + t.space_4 + t.icon_lg / 2.0, rect.center().y),
        Vec2::splat(t.icon_lg),
    );
    crate::icons::paint(
        ui.painter(),
        icon,
        icon_rect,
        t.accent.color32(),
        t.icon_stroke_lg,
    );
    let left = icon_rect.right() + t.space_4;
    ui.painter().text(
        Pos2::new(left, rect.center().y - 1.0),
        egui::Align2::LEFT_BOTTOM,
        title,
        theme::font(&t.type_body_strong),
        t.text_primary.color32(),
    );
    ui.painter().text(
        Pos2::new(left, rect.center().y + 1.0),
        egui::Align2::LEFT_TOP,
        sentence,
        theme::font(&t.type_small),
        t.text_secondary.color32(),
    );
    response
}

/// The drawing grid's step and Snap to grid (FT-1690); the tooltip names the connection grid that connection points
/// keep to.
fn grid_controls(ui: &mut Ui, editor: &SymbolEditor, events: &mut Vec<Event>) {
    use ew_viewmodel::shell::symbol_editor::GRID_STEPS;
    let words = |(num, den): (i64, i64)| ew_viewmodel::status::grid_words(num, den);
    let grid = editor.draft.symbol.grid;
    let tip = strings::format(
        key!("symbol-snap-tip"),
        &[&words((grid.num() as i64, grid.den() as i64))],
    );
    let caption = label(key!("symbol-snap-step"));
    let t = theme::current(ui.ctx());
    ui.label(
        egui::RichText::new(caption)
            .font(theme::font(&t.type_small))
            .color(t.text_secondary.color32()),
    );
    let mut step = editor.grid.min(GRID_STEPS.len() - 1);
    let combo = crate::components::choice(
        ui,
        egui::ComboBox::from_id_salt("symbol-snap-step").selected_text(words(GRID_STEPS[step])),
        |ui| {
            for (i, s) in GRID_STEPS.iter().enumerate() {
                ui.selectable_value(&mut step, i, words(*s));
            }
        },
    )
    .response;
    combo.widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, true, caption));
    let _ = combo.tip(tip.clone());
    if step != editor.grid {
        let (num, den) = GRID_STEPS[step];
        action(
            events,
            "symbol.grid",
            json!({ "step": num as f64 / den as f64 / 1e6 }),
        );
    }
    ui.add_space(t.space_2);
    let mut on = editor.snap;
    if ui
        .checkbox(&mut on, label(key!("symbol-snap")))
        .tip(tip)
        .changed()
    {
        action(events, "symbol.grid", json!({ "snap": on }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ew_viewmodel::ew_api::symbols::SymbolDraft;
    use ew_viewmodel::shell::symbol_editor::Item;

    /// A click on a text's glyphs, drawn before the rectangle around it, picks the text, the smaller item; a click on
    /// the line of the rectangle, which is not filled, picks it, a click inside it elsewhere nothing.
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
        let bounds = paint::Bounds::of(&draft, &styles);
        let text = bounds.of_item(Item::Graphic(0), 0.0).unwrap();
        assert!(text[2] - text[0] > 1.0, "the glyphs' width: {text:?}");
        // On the glyphs, away from the text's anchor point by more than the slop.
        let on_glyphs = Vec2::new(text[2] - 0.2, 10.0);
        assert_eq!(bounds.pick(on_glyphs, 0.1, 1.0), Some(Item::Graphic(0)));
        // The rectangle is not filled: it is picked on its line, not inside it.
        assert_eq!(
            bounds.pick(Vec2::new(0.05, 17.0), 0.1, 1.0),
            Some(Item::Graphic(1))
        );
        assert_eq!(bounds.pick(Vec2::new(3.0, 17.0), 0.1, 1.0), None);
        assert_eq!(bounds.pick(Vec2::new(30.0, 30.0), 0.1, 1.0), None);
    }

    /// Every variant, its large two-line text included, shows whole inside its cell, right of its letter; the
    /// text's glyphs reach past the bounds of the geometry alone.
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
        let drawn = paint::drawn_bounds(&draft, 0, &styles).unwrap();
        assert!(
            drawn[1] < geometry.min().y.0 as f32 / 1e6,
            "the text reaches above the rectangle: {drawn:?}"
        );
        let cell = Rect::from_min_size(Pos2::new(100.0, 700.0), Vec2::new(110.0, 84.0));
        let letter = Rect::from_min_size(cell.min + Vec2::new(8.0, 4.0), Vec2::new(8.0, 14.0));
        let area = Rect::from_min_max(
            Pos2::new(letter.right() + 8.0, cell.top() + 8.0),
            cell.max - Vec2::splat(8.0),
        );
        for variant in 0..8u8 {
            let b = paint::drawn_bounds(&draft, variant, &styles).unwrap();
            let (origin, scale) = paint::variant_view(b, area);
            let shown = Rect::from_min_max(
                origin + Vec2::new(b[0], b[1]) * scale,
                origin + Vec2::new(b[2], b[3]) * scale,
            );
            assert!(
                area.expand(0.01).contains_rect(shown) && !shown.intersects(letter),
                "variant {variant}: {shown:?} in {area:?}"
            );
        }
    }

    /// A slow drag of one point per frame moves a number (egui keeps the precise value between frames), and the number
    /// commits once, at the drag's end.
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
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 200.0))),
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                if fields::number(ui, value, 0.5, -100.0..=100.0, "x") {
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
        let mut value = "A".to_owned();
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
        let frame = |n: usize, events: Vec<egui::Event>, value: &mut String, commits: &mut usize| {
            let input = egui::RawInput {
                events,
                time: Some(n as f64 / 60.0),
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 200.0))),
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                if n == 0 {
                    let input = ui.make_persistent_id("text").with("input");
                    ui.memory_mut(|m| m.request_focus(input));
                }
                if panels::lines(ui, "text", "Text", value, 200.0) {
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
            (value.as_str(), commits),
            ("A", 0),
            "nothing commits while typing"
        );
        frame(4, vec![key(egui::Key::Enter, false)], &mut value, &mut commits);
        assert_eq!((value.as_str(), commits), ("A\nB", 1));
    }
}
