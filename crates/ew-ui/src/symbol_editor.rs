// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The drawing of Parts (doc 07 §5.6): the Drawing tab's command row with the way back, the drawing tools and the next
//! step; on the left the symbol's short sheet and what it consists of, in the middle the drawing, on the right
//! Properties, as in the main window. A part's box is arranged in the same place ([`device_box`]). Every edit
//! dispatches the shared headless actions.

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

/// What the window keeps of the drawing between frames: its view, a tool's half-made shape and its options.
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
    /// The drawing's top right corner in the last frame: the toasts stand under it, on the drawing, never over
    /// Properties.
    pub(crate) toast_corner: Option<Pos2>,
    /// The outline as made for the draft's generation and revision, the language and the discipline: a frame or a
    /// pointer move draws it, a change of the draft makes it again.
    outline: Option<(
        OutlineKey,
        std::rc::Rc<ew_viewmodel::shell::symbol_editor::Outline>,
    )>,
}

/// What the outline is made for: the draft's generation and revision, the language and the discipline.
type OutlineKey = (u64, u64, u64, &'static str);

/// The drawing tools of the bar: the action's word, the caption's key and the icon. Each has `symbol-tip-<word>`,
/// what it does and how.
const TOOLS: [(&str, strings::Key<'static>, strings::Key<'static>, &str); 13] = [
    (
        "select",
        key!("symbol-select"),
        key!("symbol-tip-select"),
        "pointer",
    ),
    (
        "line",
        key!("symbol-line"),
        key!("symbol-tip-line"),
        "ew-shape-line",
    ),
    (
        "rectangle",
        key!("symbol-rectangle"),
        key!("symbol-tip-rectangle"),
        "ew-shape-rectangle",
    ),
    (
        "circle",
        key!("symbol-circle"),
        key!("symbol-tip-circle"),
        "ew-shape-circle",
    ),
    (
        "ellipse",
        key!("symbol-ellipse"),
        key!("symbol-tip-ellipse"),
        "ew-shape-ellipse",
    ),
    ("arc", key!("symbol-arc"), key!("symbol-tip-arc"), "ew-shape-arc"),
    (
        "polyline",
        key!("symbol-polyline"),
        key!("symbol-tip-polyline"),
        "ew-shape-polyline",
    ),
    (
        "polygon",
        key!("symbol-polygon"),
        key!("symbol-tip-polygon"),
        "ew-shape-polygon",
    ),
    ("text", key!("symbol-text"), key!("symbol-tip-text"), "typography"),
    ("cp", key!("symbol-cp"), key!("symbol-tip-cp"), "plug-connected"),
    ("slot", key!("symbol-slot"), key!("symbol-tip-slot"), "text-size"),
    (
        "anchor",
        key!("symbol-anchor"),
        key!("symbol-tip-anchor"),
        "current-location",
    ),
    ("link", key!("symbol-link"), key!("symbol-tip-link"), "link"),
];

/// A command of the editor's bar, with the main bar's sizing, painting and accessible name: `id` the action it runs (a
/// tool and a menu, which no action names alone, by the key of its caption), greyed with `refusal`, the shell's reason;
/// `how` is one line on what it does, under its name in the tooltip.
fn bar_command(
    ui: &mut Ui,
    (id, key, icon): (&'static str, strings::Key<'static>, &'static str),
    size: CommandSize,
    [checked, menu]: [bool; 2],
    refusal: Option<String>,
    shortcut: String,
    how: Option<&str>,
) -> egui::Response {
    let name = label(key);
    crate::command_bar::inline_command(
        ui,
        &CommandButton {
            id,
            label: how.map_or_else(|| name.to_owned(), |how| format!("{name}\n{how}")),
            bar_label: name.to_owned(),
            icon,
            size,
            shortcut,
            enabled: refusal.is_none(),
            reason: refusal,
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
        let Some((word, caption, tip, icon)) = TOOLS.iter().find(|(w, ..)| *w == word).copied() else {
            return false;
        };
        // The tooltip names the key of the keymap that chooses the tool.
        let key = TOOL_KEYS
            .iter()
            .find(|(tool, _)| *tool == word)
            .and_then(|(_, action)| ew_viewmodel::actions::find(action))
            .map(|m| shell.shortcut_text(m))
            .unwrap_or_default();
        // What the tool does and how, under its name.
        let how = label(tip);
        let clicked = bar_command(
            ui,
            (caption.as_str(), caption, icon),
            size,
            [editor.tool == word, false],
            None,
            key,
            Some(how),
        )
        .clicked();
        if clicked {
            action(events, "symbol.tool", json!({ "tool": word }));
        }
        clicked
    }

    /// The Drawing tab's command row (doc 07 §5.1): the way back; Select, the shapes and Text; Pin, Text field and the
    /// other marks; the drawing grid and Snap; Test on a page; and at the right end the next step, its key beside it.
    pub(crate) fn toolbar(&mut self, ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
        self.sync(shell);
        let Some(editor) = shell.symbol_editor() else {
            return;
        };
        let t = theme::current(ui.ctx());
        ui.spacing_mut().item_spacing = Vec2::new(t.space_1, t.space_1);
        if let Some(b) = editor.device_box() {
            self.box_toolbar(ui, shell, editor, b, events);
            return;
        }
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            back_command(ui, editor, events);
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
                    (
                        key!("symbol-more-shapes").as_str(),
                        key!("symbol-more-shapes"),
                        "category",
                    ),
                    CommandSize::Small,
                    [more.contains(&editor.tool), true],
                    None,
                    String::new(),
                    None,
                );
                let items = tool_items(shell, editor, &more);
                egui::Popup::menu(&menu).show(|ui| crate::menu::items(ui, &items, events));
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
                    (
                        key!("symbol-more-marks").as_str(),
                        key!("symbol-more-marks"),
                        "dots",
                    ),
                    CommandSize::Small,
                    [more.contains(&editor.tool), true],
                    None,
                    String::new(),
                    None,
                );
                let items = tool_items(shell, editor, &more);
                egui::Popup::menu(&menu).show(|ui| crate::menu::items(ui, &items, events));
            });
            divider(ui);
            grid_controls(ui, editor, events);
            divider(ui);
            let scratch = refusal(shell, "symbol.scratch");
            let can_scratch = scratch.is_none();
            stacked(ui, |ui| {
                if bar_command(
                    ui,
                    ("symbol.scratch", key!("symbol-scratch"), "flask"),
                    CommandSize::Small,
                    [false, false],
                    scratch,
                    String::new(),
                    Some(label(key!("symbol-scratch-how"))),
                )
                .clicked()
                    && can_scratch
                {
                    action(events, "symbol.scratch", json!({}));
                }
                if bar_command(
                    ui,
                    ("symbol.export", key!("symbol-export"), "file-arrow-right"),
                    CommandSize::Small,
                    [false, false],
                    None,
                    String::new(),
                    None,
                )
                .clicked()
                {
                    action(events, "symbol.export", json!({}));
                }
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                next_step(ui, shell, editor.save_label(), events);
            });
        });
    }
}

/// The shell's reason why action `id` cannot run now, in words.
fn refusal(shell: &ShellModel, id: &str) -> Option<String> {
    ew_viewmodel::actions::find(id)
        .and_then(|m| m.enabled_now(shell).err())
        .map(|r| strings::render(&r))
}

/// The Drawing tab's way back, at its left end: Back without it from a page, To the part from a piece or a box.
fn back_command(ui: &mut Ui, editor: &SymbolEditor, events: &mut Vec<Event>) {
    let words = editor.back_label();
    if crate::command_bar::inline_command(
        ui,
        &CommandButton {
            id: "parts.back",
            label: words.to_owned(),
            bar_label: words.to_owned(),
            icon: "arrow-left",
            size: CommandSize::Large,
            shortcut: String::new(),
            enabled: true,
            reason: None,
            checked: false,
            topic: Some("symbol-editor"),
            kind: ButtonKind::Plain,
        },
    )
    .clicked()
    {
        action(events, "parts.back", json!({}));
    }
}

/// The Drawing tab's next step at its right end (doc 16a §5.2): the one Primary button, `words` (Save and place on
/// 102, Save and use for -102S1, Done, Save), greyed with the shell's reason, its key beside it.
fn next_step(ui: &mut Ui, shell: &ShellModel, words: String, events: &mut Vec<Event>) {
    let t = theme::current(ui.ctx());
    let why = refusal(shell, "parts.save");
    if components::button(ui, Kind::Primary, &words, why.as_deref(), 0.0).clicked() && why.is_none() {
        action(events, "parts.save", json!({}));
    }
    let key = ew_viewmodel::actions::find("parts.save")
        .map(|m| shell.shortcut_text(m))
        .unwrap_or_default();
    if !key.is_empty() {
        ui.add_space(t.space_2);
        components::caption_chip(ui, &ew_viewmodel::keymap::short_display(&key));
    }
}

impl EditorView {

    /// The drawing in the middle of Parts: the short sheet and the outline, the drawing and Properties.
    pub fn show(&mut self, ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
        self.sync(shell);
        let Some(editor) = shell.symbol_editor() else {
            return;
        };
        let t = theme::current(ui.ctx());
        let full = ui.max_rect();
        ui.allocate_rect(full, Sense::hover());
        // F1 on a part of the screen (the outline, the drawing, Properties, the box): its section of the drawing's
        // page.
        let help = |ui: &Ui, rect: Rect, part: ew_viewmodel::help::EditorPart| {
            crate::help::offer(ui, rect, || {
                let topic = ew_viewmodel::help::symbol_editor_part(editor, part);
                (topic.to_owned(), strings::text(key!("symbol-title")).to_owned())
            });
        };
        help(ui, full, ew_viewmodel::help::EditorPart::Other);
        let styles = shell.symbol_text_styles();
        // The side panels keep their width; the drawing takes the room (doc 16a §2).
        let left_w = (t.size_panel_left + 2.0 * t.space_4)
            .min(full.width() * crate::theme::shares::SYMBOL_PANEL_SHARE);
        let right_w = (t.size_panel_left + t.size_property_label)
            .min(full.width() * crate::theme::shares::SYMBOL_PANEL_SHARE);
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
                self.box_drawing(ui, centre, editor, b, events);
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
        let mut top = crate::parts_place::drawing_strip(ui, rect, editor, events);
        // Why the last action did nothing, above the drawing: a refused Save or Test must answer where the eye is.
        if let Some(error) = &editor.error {
            let banner = ui.scope_builder(
                UiBuilder::new().max_rect(Rect::from_min_max(
                    Pos2::new(rect.left(), top),
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
                            // Greyed where the shell refuses the offer, with its reason.
                            let refusal = ew_viewmodel::actions::find(offer)
                                .and_then(|m| m.enabled_now(shell).err())
                                .map(|r| strings::render(&r));
                            if editor.offer_new_page
                                && components::button(
                                    ui,
                                    Kind::Secondary,
                                    label(text),
                                    refusal.as_deref(),
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
        ui.label(crate::components::small_secondary(
            &t,
            label(key!("symbol-variants")),
        ));
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
                        ui.label(crate::components::small_secondary(
                            &t,
                            TOOLS
                                .iter()
                                .find(|(w, ..)| *w == editor.tool)
                                .map_or("", |(_, _, tip, _)| label(*tip)),
                        ));
                    });
                });
            });
    }

    /// The left panel (doc 07 §5.6): the symbol's short sheet (Name, What it is, where it is listed, the grid of its
    /// pins), the line that says why it is not his to change, what it consists of (each row a click to the thing),
    /// and what is left to do.
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
            short_sheet(ui, editor, events);
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
                        ui.label(crate::components::small_tertiary(&t, group.empty.clone()));
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
                            "parts.field",
                            json!({ "field": "symbol.reviewed", "value": reviewed.to_string() }),
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
            ui.push_id(salt, |ui| {
                panels::sheet_rows(ui, &sheet.rows, &sheet.title, "symbol.prop", events, |_, _| {});
            });
            ui.add_space(t.space_3);
            if editor.selection.is_empty() {
                return;
            }
            // What is done with the selection, beside its keys.
            ui.horizontal_wrapped(|ui| {
                if editor.selection.len() > 1 {
                    let line_up =
                        components::button(ui, Kind::Secondary, label(key!("symbol-line-up")), None, 0.0);
                    let items = [
                        ("left", key!("symbol-line-up-left")),
                        ("centre", key!("symbol-line-up-centre")),
                        ("right", key!("symbol-line-up-right")),
                        ("top", key!("symbol-line-up-top")),
                        ("middle", key!("symbol-line-up-middle")),
                        ("bottom", key!("symbol-line-up-bottom")),
                    ]
                    .map(|(to, words)| ew_viewmodel::actions::ActionItem {
                        id: "symbol.align",
                        params: json!({ "to": to }),
                        label: label(words).to_owned(),
                        shortcut: String::new(),
                        enabled: true,
                        reason: None,
                        checked: false,
                        topic: None,
                    });
                    egui::Popup::menu(&line_up).show(|ui| crate::menu::items(ui, &items, events));
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

}

/// The symbol's short sheet (doc 07 §5.6): Name and What it is, typed and chosen where they stand (`parts.field`), and
/// what follows from them, not asked: where it is listed and the grid of its pins; a symbol that is not his reads as
/// text, with one line that says why and Make my own from it.
fn short_sheet(ui: &mut Ui, editor: &SymbolEditor, events: &mut Vec<Event>) {
    let t = theme::current(ui.ctx());
    if let Some(why) = &editor.locked {
        egui::Frame::NONE
            .fill(t.surface_sunken.color32())
            .corner_radius(theme::radius(t.radius_md))
            .inner_margin(egui::Margin::symmetric(t.space_3 as i8, t.space_2 as i8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.add(egui::Label::new(why).wrap());
                if components::button(ui, Kind::Secondary, label(key!("parts-own-from")), None, 0.0).clicked() {
                    action(events, "parts.own", json!({}));
                }
            });
        ui.add_space(t.space_2);
    }
    let rows = editor.symbol_rows();
    let from = events.len();
    panels::sheet_rows(ui, &rows, &label(key!("symbol-the-symbol")), "symbol.prop", events, |_, _| {});
    // A field of the short sheet is the symbol's own (`parts.field symbol.<key>`), not a selected thing's.
    for e in &mut events[from..] {
        if let Event::Action(name, params) = e
            && *name == "symbol.prop"
        {
            let key = params["key"].as_str().unwrap_or_default().to_owned();
            let value = match &params["value"] {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            *e = Event::Action("parts.field", json!({ "field": format!("symbol.{key}"), "value": value }));
        }
    }
    let grid = editor.draft.symbol.grid;
    let pins_on = ew_viewmodel::status::grid_words(grid.num() as i64, grid.den() as i64);
    for (caption, value) in [
        (label(key!("symbol-category")), editor.listed_under()),
        (label(key!("symbol-pins-on")), pins_on),
    ] {
        ui.horizontal(|ui| {
            ui.label(crate::components::small_secondary(&t, caption));
            ui.label(value);
        });
    }
    ui.add_space(t.space_2);
}

/// The drawing grid's step and Snap to grid; the tooltip names the connection grid that connection points
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
    ui.label(crate::components::small_secondary(&t, caption));
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

/// The tools `words` as the rows of a button's menu ([`crate::menu::items`]): each with its caption and the key of the
/// keymap that chooses it, the one in use checked; a row chooses its tool (`symbol.tool`).
fn tool_items(
    shell: &ShellModel,
    editor: &SymbolEditor,
    words: &[&str],
) -> Vec<ew_viewmodel::actions::ActionItem> {
    words
        .iter()
        .filter_map(|word| TOOLS.iter().find(|(w, ..)| w == word))
        .map(|(word, caption, _, _)| ew_viewmodel::actions::ActionItem {
            id: "symbol.tool",
            params: json!({ "tool": word }),
            label: label(*caption).to_owned(),
            shortcut: TOOL_KEYS
                .iter()
                .find(|(tool, _)| tool == word)
                .and_then(|(_, action)| ew_viewmodel::actions::find(action))
                .map(|m| shell.shortcut_text(m))
                .unwrap_or_default(),
            enabled: true,
            reason: None,
            checked: editor.tool == *word,
            topic: None,
        })
        .collect()
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
