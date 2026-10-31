// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The property grid pane (WP-S2.05 part 1, spec §6.9): the shell's sheet drawn as the inline preview card, the header
//! and one block per section with collapsible groups; each row its label, its value or editor, the provenance chip
//! (an icon glyph and words, never colour alone) and its actions. The toast of a commit with several targets is one of
//! the window's toasts since WP-S2.09 part 1 (`crate::notifications`).
//!
//! WP-S2.17 part 3 (D11; doc 16a §6.8): the header's title in `type.title` and its subtitle in `type.small`; the Main
//! group (the kind's primary fields) first, then the categories; the Advanced rows (`$v`, ids, source references) only
//! after "Show advanced (n)" at the foot. A value cell edits in place (a click, with a hover border); an empty value
//! shows an em dash; the provenance chip only for a value that is not authored, its whole text in the tooltip. The
//! row's actions are one "…" button (the actions menu, `props.row_menu`), shown on the cursor row, on hover and on
//! focus: no edit buttons.
//!
//! **Input.** Every gesture is a registry action (`props.*`, `preview.*`); the pane builds no command.
//! While the grid has the shell's focus and no field is being edited: Up, Down, Home and End move the row cursor,
//! Left and Right collapse and expand the focused category, Enter or F2 edits, typing a character starts the edit with
//! it, Insert, Delete, Alt+Up and Alt+Down change a list. In the field: Enter or Tab commit (as does leaving it),
//! Escape reverts, Shift+Enter opens the multi-language popup.
//!
//! **Accessibility.** A row is a node with the role `Group`, named by its label, with the value, the provenance and
//! the error in its description; the editor is a text input labelled by the field; the card is a polite live region.
//! The pane announces nothing on a selection change.

use egui::{Key, Modifiers, RichText, Sense, TextEdit, Ui, Vec2};
use ew_viewmodel::properties::{
    ChoiceKind, FieldType, Popup, PropRow, PropertySheet, Provenance, RowAction, RowValue, SheetSection,
    advanced_count, chip_text, chip_word, group_key, groups, is_header, read_only_reason, row_label,
    row_matches, value_text,
};
use ew_viewmodel::shell::{Focus, ShellModel};
use ew_viewmodel::strings;
use serde_json::json;

use crate::app::Event;
use crate::components::Tip as _;

/// A text longer than this many characters is ellipsised in the middle (the whole text in the tooltip).
const MAX_SHOWN: usize = 48;

/// The grid pane's state between frames.
#[derive(Default)]
pub struct PropertiesPane {
    /// The editor's text (the shell's buffer is set from it).
    buffer: String,
    /// The row the editor was opened for.
    editing: Option<String>,
    /// The multi-language popup's texts by language (`None`: neutral).
    mltext: Vec<(Option<String>, String)>,
    /// The options list box of the row being edited and where it was drawn in the last frame: a pointer press on it
    /// takes the field's focus, and that focus loss must not commit the buffer before the click chooses.
    choices_rect: Option<(String, egui::Rect)>,
    /// The nesting level of the shallowest rows of the group being drawn (their labels are not indented).
    base_depth: u8,
    /// The width the widest label of the rows on show needs this frame (M39: the label column fits its labels).
    label_need: f32,
    /// The filter field's text (FT-541; the shell's filter is set from it).
    filter: String,
    /// The shell's filter text as last seen: a change made elsewhere (`props.filter`, Clear filter) shows in the field
    /// even while a text field has the focus.
    filter_seen: String,
}

/// `text` ellipsised in the middle at [`MAX_SHOWN`] characters.
fn middle(text: &str) -> String {
    let n = text.chars().count();
    if n <= MAX_SHOWN {
        return text.to_owned();
    }
    let head: String = text.chars().take(MAX_SHOWN / 2 - 1).collect();
    let tail: String = text.chars().skip(n - (MAX_SHOWN / 2 - 1)).collect();
    format!("{head}…{tail}")
}

impl PropertiesPane {
    /// A pane with no editor.
    pub fn new() -> PropertiesPane {
        PropertiesPane::default()
    }

    /// Paints the grid of `shell` into `ui`.
    pub(crate) fn show(&mut self, ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
        let props = shell.properties();
        // The pane's own focusable node: the grid's keys while it has the shell's focus.
        let rect = ui.available_rect_before_wrap();
        let pane_id = ui.scope_id().with("property-grid");
        let pane = ui.interact(rect, pane_id, Sense::focusable_noninteractive());
        let pane_name = strings::text("pane-properties").to_owned();
        pane.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Group, true, &pane_name));
        let ours = shell.focus() == Some(Focus::Properties);
        // A field is being typed into (its own keys), unless choices wait for a number key or a popup is open; a choice
        // editor's options keep the field focused (Up, Down and Enter are the field's).
        let options = props.choices().is_some_and(|c| c.kind == ChoiceKind::Options);
        let editing =
            props.edit().is_some() && (props.choices().is_none() || options) && props.popup().is_none();
        if ours && !editing && !pane.has_focus() && !ui.ctx().text_edit_focused() && shell.dialog().is_none()
        {
            pane.request_focus();
        }
        if pane.has_focus() {
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(
                    pane_id,
                    egui::EventFilter {
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        tab: false,
                        escape: false,
                    },
                );
            });
            if !editing {
                self.keys(ui, shell, events);
            }
        }
        if pane.gained_focus() && !ours {
            events.push(Event::Action("ui.focus", json!({"area": "properties"})));
        }
        ui.scope_builder(egui::UiBuilder::new().accessibility_parent(pane_id), |ui| {
            self.card(ui, shell, events);
            let Some(sheet) = props.sheet() else {
                ui.label(strings::text("props-empty"));
                return;
            };
            Self::header(ui, sheet, shell.editing_folio().as_deref(), events);
            // FT-541 (doc 16a §6.8): "Filter properties" under the header while the open categories hold more than
            // twelve rows.
            let filter = shell.properties_filter().to_owned();
            if shell.properties_filter_shown() {
                self.filter_field(ui, shell, events);
            }
            if sheet.sections.is_empty() {
                let text = if sheet.missing > 0 && sheet.header.items == 0 {
                    strings::text("props-gone-title").to_owned()
                } else if sheet.header.items == 0 {
                    strings::text("props-empty").to_owned()
                } else {
                    let kinds: Vec<String> = sheet.header.kinds.iter().map(|(k, _)| k.clone()).collect();
                    strings::format("props-none", &[&kinds.join(", ")])
                };
                ui.label(text);
                return;
            }
            let focused = shell.properties_row();
            let collapsed = shell.properties_collapsed();
            let advanced = shell.properties_show_advanced();
            let t = crate::theme::current(ui.ctx());
            // A filter that hides every row says so, with Clear filter (doc 16a §6.17).
            if !filter.is_empty()
                && ew_viewmodel::properties::visible_rows(sheet, &collapsed, advanced, &filter).is_empty()
            {
                let line = strings::format("empty-nav-filter", &[&filter]);
                let clear = crate::components::empty_state(
                    ui,
                    crate::icons::IconId::filter,
                    &line,
                    strings::text("empty-nav-filter-text"),
                    Some(strings::text("empty-nav-clear")),
                );
                if clear.is_some_and(|r| r.clicked()) {
                    self.filter.clear();
                    events.push(Event::Action("props.filter", json!({ "text": "" })));
                }
                return;
            }
            // M39: the label column fits the widest label of the rows on show (with its indent).
            self.label_need = {
                let font = crate::theme::font(&t.type_small);
                let mut need = 0.0_f32;
                for section in &sheet.sections {
                    for (group, rows) in groups(section, advanced) {
                        if collapsed.contains(&group_key(&section.kind, group)) {
                            continue;
                        }
                        let rows: Vec<&PropRow> = rows
                            .into_iter()
                            .filter(|r| row_matches(section, r, &filter))
                            .collect();
                        let base = if matches!(group, ew_viewmodel::properties::RowGroup::Main) {
                            u8::MAX
                        } else {
                            rows.iter().map(|r| r.depth).min().unwrap_or(0)
                        };
                        for row in rows {
                            let depth = t.space_3 * f32::from(row.depth.saturating_sub(base)).min(6.0);
                            let w = ui
                                .painter()
                                .layout_no_wrap(row_label(section, row), font.clone(), crate::theme::NONE)
                                .size()
                                .x;
                            need = need.max(depth + w);
                        }
                    }
                }
                need
            };
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for section in &sheet.sections {
                        if sheet.sections.len() > 1 {
                            ui.label(
                                RichText::new(format!(
                                    "{} ({})",
                                    strings::kind_plural(&section.kind),
                                    section.targets.len()
                                ))
                                .font(crate::theme::font(&t.type_body_strong)),
                            );
                        }
                        for (group, rows) in groups(section, advanced) {
                            let key = group_key(&section.kind, group);
                            let shut = collapsed.contains(&key);
                            // FT-541: the filter keeps the rows it matches; an open group left without rows hides.
                            let rows: Vec<&PropRow> = rows
                                .into_iter()
                                .filter(|r| row_matches(section, r, &filter))
                                .collect();
                            if !shut && rows.is_empty() {
                                continue;
                            }
                            if group_heading(ui, &t, group.title(), &key, shut) {
                                events.push(Event::Action("ui.focus", json!({"area": "properties"})));
                                events.push(Event::Action(
                                    "props.category",
                                    json!({"category": group.word(), "expanded": shut}),
                                ));
                            }
                            if shut {
                                continue;
                            }
                            // A category's shallowest rows are its first level: the label indent counts from them.
                            // Main gathers fields from several levels without their parent rows, so it indents none.
                            self.base_depth = if matches!(group, ew_viewmodel::properties::RowGroup::Main) {
                                u8::MAX
                            } else {
                                rows.iter().map(|r| r.depth).min().unwrap_or(0)
                            };
                            for row in rows {
                                self.row(ui, shell, sheet, section, row, focused.as_deref(), events);
                            }
                        }
                    }
                    // The foot: "Show advanced (n)" or "Hide advanced (n)".
                    let n = advanced_count(sheet);
                    if n > 0 {
                        ui.add_space(t.space_2);
                        let key = if advanced {
                            "props-hide-advanced"
                        } else {
                            "props-show-advanced"
                        };
                        let text = strings::format(key, &[&n.to_string()]);
                        let link = ui.add(
                            egui::Button::new(
                                RichText::new(text)
                                    .color(t.accent.color32())
                                    .font(crate::theme::font(&t.type_small)),
                            )
                            .frame(false),
                        );
                        if link.clicked() {
                            events.push(Event::Action("props.show_advanced", json!({ "on": !advanced })));
                        }
                    }
                });
        });
    }

    /// The filter field (FT-541; doc 16a §6.5, §6.8): "Filter properties", inset `space.4` from the pane's sides and
    /// `size.control` high, as the navigator's filter; a change is `props.filter`.
    fn filter_field(&mut self, ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
        let t = crate::theme::current(ui.ctx());
        let wanted = shell.properties().filter();
        if wanted != self.filter_seen || (self.filter != wanted && !ui.ctx().text_edit_focused()) {
            self.filter = wanted.to_owned();
            self.filter_seen = wanted.to_owned();
        }
        let line = {
            let free = ui.available_rect_before_wrap();
            egui::Rect::from_min_size(free.min, Vec2::new(free.width(), t.size_control))
                .shrink2(Vec2::new(t.space_4, 0.0))
        };
        let hint = strings::text("props-filter-hint");
        let field = ui
            .scope_builder(
                egui::UiBuilder::new()
                    .max_rect(line)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
                |ui| {
                    ui.set_min_size(line.size());
                    crate::components::filter_field(ui, &mut self.filter, hint)
                },
            )
            .inner;
        field.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, hint));
        ui.add_space(t.space_2);
        if field.changed() {
            self.filter_seen = self.filter.clone();
            events.push(Event::Action("props.filter", json!({ "text": self.filter })));
        }
    }

    /// The grid's keys while it has the focus and no field is edited (§6.9).
    fn keys(&self, ui: &Ui, shell: &ShellModel, events: &mut Vec<Event>) {
        let pressed = |m: Modifiers, k: Key| ui.ctx().input_mut(|i| i.consume_key(m, k));
        let row = shell.properties_row();
        // The actions popup: Up, Down and Enter.
        if let Some(ew_viewmodel::properties::Popup::Actions { index, .. }) = shell.properties().popup() {
            if pressed(Modifiers::NONE, Key::ArrowDown) {
                events.push(Event::Action("props.row_menu", json!({"step": 1})));
            } else if pressed(Modifiers::NONE, Key::ArrowUp) {
                events.push(Event::Action("props.row_menu", json!({"step": -1})));
            } else if pressed(Modifiers::NONE, Key::Enter) {
                events.push(Event::Action("props.choose", json!({ "index": index })));
            }
            return;
        }
        // The choices under a field: their number keys (1 to 9) choose.
        if let Some(c) = shell.properties().choices() {
            let keys = [
                Key::Num1,
                Key::Num2,
                Key::Num3,
                Key::Num4,
                Key::Num5,
                Key::Num6,
                Key::Num7,
                Key::Num8,
                Key::Num9,
            ];
            for (i, k) in keys.iter().enumerate().take(c.items.len()) {
                if pressed(Modifiers::NONE, *k) {
                    events.push(Event::Action("props.choose", json!({ "index": i })));
                    return;
                }
            }
        }
        if pressed(Modifiers::ALT, Key::ArrowUp) {
            events.push(Event::Action("props.list", json!({"row": row, "op": "up"})));
        } else if pressed(Modifiers::ALT, Key::ArrowDown) {
            events.push(Event::Action("props.list", json!({"row": row, "op": "down"})));
        } else if pressed(Modifiers::NONE, Key::ArrowDown) {
            events.push(Event::Action("props.row", json!({"step": 1})));
        } else if pressed(Modifiers::NONE, Key::ArrowUp) {
            events.push(Event::Action("props.row", json!({"step": -1})));
        } else if pressed(Modifiers::NONE, Key::Home) {
            events.push(Event::Action("props.row", json!({"to": "first"})));
        } else if pressed(Modifiers::NONE, Key::End) {
            events.push(Event::Action("props.row", json!({"to": "last"})));
        } else if pressed(Modifiers::NONE, Key::ArrowLeft) {
            events.push(Event::Action("props.category", json!({"expanded": false})));
        } else if pressed(Modifiers::NONE, Key::ArrowRight) {
            events.push(Event::Action("props.category", json!({"expanded": true})));
        } else if pressed(Modifiers::NONE, Key::Enter) {
            if shell.properties().card().is_none() {
                events.push(Event::Action("props.edit", json!({})));
            }
        } else if pressed(Modifiers::NONE, Key::Space) {
            // Space toggles a boolean row's checkbox.
            if let Some((key, on)) = row.as_deref().and_then(|k| bool_row(shell, k)) {
                events.push(Event::Action(
                    "props.set",
                    json!({"row": key, "value": if on { "no" } else { "yes" }}),
                ));
            }
        } else if pressed(Modifiers::NONE, Key::Insert) {
            events.push(Event::Action("props.list", json!({"row": row, "op": "add"})));
        } else if pressed(Modifiers::NONE, Key::Delete) {
            events.push(Event::Action("props.list", json!({"row": row, "op": "remove"})));
        } else {
            // A typed character on a focused editable row starts the edit with it.
            let typed = ui.ctx().input_mut(|i| {
                let mut text = None;
                i.events.retain(|e| match e {
                    egui::Event::Text(t) if text.is_none() && !t.trim().is_empty() => {
                        text = Some(t.clone());
                        false
                    }
                    _ => true,
                });
                text
            });
            if let Some(t) = typed {
                events.push(Event::Action("props.input", json!({ "text": t })));
            }
        }
    }

    /// The header (§6.2): title, subtitle, location, finding chips (doc 16a §6.8: the title in `type.title`, the
    /// subtitle and the location in `type.small`, secondary; FT-530: one severity chip per finding at the right of the
    /// title line, its icon and word, the rule and message as the tooltip and in its node's name).
    fn header(ui: &mut Ui, sheet: &PropertySheet, editing: Option<&str>, events: &mut Vec<Event>) {
        let t = crate::theme::current(ui.ctx());
        // Cycle 36 (window check W36): the chips are laid out first, at the right, and the title takes what is left,
        // ellipsised; the findings collapse into one count chip per severity ("3 Warnings", every rule and message in
        // its tooltip), so the header keeps its width whatever the count (doc 16a §6.8: "the findings badge").
        // Cycle 37 (window check W37): always one chip per severity, also for one or two findings ("2 Warnings", not
        // "Warning Warning").
        // FT-664 (cycle 37): the chips never take the title's room. When the title at its full width and the chips
        // with their words do not fit the line, each severity becomes one glyph chip (its icon, and the count when
        // more than one), the words in its tooltip and node name; the title is then ellipsised by the panel only.
        let findings = &sheet.header.findings;
        let groups = severity_groups(findings, editing);
        let compact = {
            let measure = |text: String, font: &crate::theme::TypeToken| {
                ui.painter()
                    .layout_no_wrap(text, crate::theme::font(font), t.text_primary.color32())
                    .size()
                    .x
            };
            let chip = |word: String| {
                t.space_2
                    + t.type_caption.size
                    + t.space_1
                    + t.space_1
                    + measure(word, &t.type_caption)
                    + t.space_2
            };
            let chips: f32 = groups
                .iter()
                .map(|(severity, of)| chip(count_words(severity, of.len())))
                .sum();
            let gaps = t.space_1 * groups.len().saturating_sub(1) as f32;
            // C39 (window check W39c2): the title elides first (whole in its tooltip), down to its first three
            // characters and the ellipsis; only past that do the chips lose their words.
            let shortest: String = sheet
                .header
                .title
                .chars()
                .take(3)
                .chain(std::iter::once('\u{2026}'))
                .collect();
            let title = measure(shortest, &t.type_title);
            !findings.is_empty() && title + t.space_2 + chips + gaps > ui.available_width() - 2.0 * t.space_4
        };
        // C39: the header keeps the pane's `space.4` gutter on both sides, as the filter field and the row labels, so
        // a chip at the right end is never clipped by the pane's edge.
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(crate::theme::margin_px(t.space_4), 0))
            .show(ui, |ui| Self::header_lines(ui, sheet, &groups, compact, events));
        ui.separator();
    }

    /// The header's lines inside its gutter: the title line with its chips, the subtitle, the location, the read-only
    /// reason ([`Self::header`]).
    fn header_lines(
        ui: &mut Ui,
        sheet: &PropertySheet,
        groups: &[(&str, Vec<(&str, &str)>)],
        compact: bool,
        events: &mut Vec<Event>,
    ) {
        let t = crate::theme::current(ui.ctx());
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = t.space_1;
                // Right to left: info, warning, error, so the most severe reads first. FT-759: a chip is a button
                // that opens Messages filtered to the record (the existing `messages.show` and `navigator.filter`).
                for (severity, of) in groups.iter().rev() {
                    let chip = if compact {
                        glyph_chip(ui, severity, of)
                    } else {
                        crate::messages::severity_count_chip(ui, severity, of)
                    };
                    if chip.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        events.extend(findings_events(&sheet.header));
                    }
                }
                ui.add_space(t.space_2);
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.add(
                        egui::Label::new(
                            RichText::new(&sheet.header.title).font(crate::theme::font(&t.type_title)),
                        )
                        .truncate(),
                    );
                });
            });
        });
        let small = |s: &str| {
            RichText::new(s)
                .font(crate::theme::font(&t.type_small))
                .color(t.text_secondary.color32())
        };
        if let Some(s) = &sheet.header.subtitle {
            ui.label(small(s));
        }
        if let Some(l) = &sheet.header.location {
            ui.label(small(l));
        }
        if let Some(r) = &sheet.read_only {
            ui.label(RichText::new(r).italics());
        }
    }

    /// The inline preview card (§6.6; WP-S2.17 part 4, doc 16a §6.23): "Preview of `<command>`" with its revision and
    /// status badge, the summary as a polite live region, the first tag changes, Apply and Cancel.
    fn card(&self, ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
        let Some(card) = shell.properties().card() else {
            return;
        };
        // A replace card is the find pane's (WP-S2.05 part 2).
        if !matches!(card.origin, ew_viewmodel::properties::CardOrigin::Properties) {
            return;
        }
        let kind = card.edit.kind.clone();
        let title = strings::format(
            "card-title-properties",
            &[strings::command_title(&card.preview.name)],
        );
        let summary = strings::format(
            "card-line",
            &[
                &card.targets.to_string(),
                &ew_viewmodel::properties::kind_plural(&kind, card.targets),
                &card.changes.to_string(),
                &card.preview.derived.tags.len().to_string(),
                &card.preview.derived.xrefs.len().to_string(),
            ],
        );
        let tags: Vec<String> = card
            .preview
            .derived
            .tags
            .iter()
            .take(20)
            .map(|tag| format!("{} → {}", tag.before, tag.after))
            .collect();
        let inline = crate::components::InlineCard {
            title: &title,
            summary: &summary,
            apply: "preview.apply",
            strip: true,
        };
        crate::components::inline_card(ui, shell, inline, events, |ui, _| {
            for tag in &tags {
                ui.label(tag);
            }
        });
    }

    /// One row: a `Group` node with the label, the value or the editor, the chip and the actions.
    #[allow(clippy::too_many_arguments)]
    fn row(
        &mut self,
        ui: &mut Ui,
        shell: &ShellModel,
        sheet: &PropertySheet,
        section: &SheetSection,
        row: &PropRow,
        focused: Option<&str>,
        events: &mut Vec<Event>,
    ) {
        let props = shell.properties();
        let label = row_label(section, row);
        let header = is_header(section, row);
        let value = if header {
            String::new()
        } else if is_page_kind(section, row) {
            // M39 (doc 16a: words, not IDs): a page's Kind in the New page dialog's words ("Multi-line").
            ew_viewmodel::strings::choice_text(&value_text(row))
        } else {
            value_text(row)
        };
        let reason = read_only_reason(sheet, section, row);
        let chip = if header {
            String::new()
        } else {
            chip_text(&row.provenance)
        };
        // D11: the chip stands only by a value that is not authored.
        let show_chip = !header && !matches!(row.provenance, Provenance::Authored);
        let error = props
            .error()
            .filter(|(r, _)| *r == row.key)
            .map(|(_, m)| m.to_owned());
        let mut description = format!("{value}; {chip}");
        if let Some(r) = &reason {
            description.push_str(&format!("; {r}"));
        }
        if let Some(e) = &error {
            description.push_str(&format!("; {e}"));
        }
        // Doc 16a §6.8 (the mockup's `.prow`): rows of `size.row` with `space.4` side padding; the label left-aligned
        // in `type.small` and `color.text.secondary` in the `size.property_label` column, ellipsised with the whole
        // label in the tooltip; `space.3` to the value cell (`size.control.inline`); a nested row is indented by
        // `space.3` per level (16a sets no property indent).
        let t = crate::theme::current(ui.ctx());
        let width = ui.available_width();
        // M39 (owner 2026-10-31: scale, not cap): the label column fits the widest label on show, between
        // `size.property_label` and 40 % of the row; a label that still does not fit elides with the whole label in
        // its tooltip.
        let (row_height, label_width, indent) =
            (t.size_row, label_column(&t, width, self.label_need), t.space_3);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, row_height), Sense::hover());
        let group_id = ui.scope_id().with(("prop-row", &row.key));
        let group = ui.interact(rect, group_id, Sense::hover());
        let name = label.clone();
        group.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Group, true, &name));
        let desc = description.clone();
        ui.ctx().accesskit_node_builder(group_id, |n| {
            n.set_description(desc);
        });
        // Doc 16a §6.8: a right-click anywhere on the row opens the same actions menu as its "…" button (on the
        // cursor row, so the menu anchors on that button).
        let right_click = ui.input(|i| {
            i.pointer.button_clicked(egui::PointerButton::Secondary)
                && i.pointer.interact_pos().is_some_and(|p| rect.contains(p))
        });
        if right_click && !row.actions.is_empty() && ui.is_rect_visible(rect) {
            events.push(Event::Action("ui.focus", json!({"area": "properties"})));
            events.push(Event::Action("props.row", json!({"to": row.key})));
            events.push(Event::Action("props.row_menu", json!({})));
        }
        let selected = focused == Some(row.key.as_str());
        // The cursor row: the Selected state of a tree row (§6.6), `color.accent.subtle` and the 2-px accent bar.
        if selected {
            ui.painter()
                .rect_filled(rect, crate::theme::SQUARE, t.accent_subtle.color32());
            let bar = egui::Rect::from_min_max(
                egui::pos2(rect.left(), rect.top() + t.space_1),
                egui::pos2(rect.left() + t.focus_width, rect.bottom() - t.space_1),
            );
            ui.painter()
                .rect_filled(bar, crate::theme::radius(t.radius_mark), t.accent.color32());
        }
        // Tertiary parts on `color.accent.subtle` are drawn in `color.text.secondary` (§4.15).
        let tertiary = if selected {
            t.text_secondary
        } else {
            t.text_tertiary
        };
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(rect.shrink2(Vec2::new(t.space_4, 0.0)))
                .accessibility_parent(group_id)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        child.spacing_mut().interact_size.y = t.size_control_inline;
        child.spacing_mut().item_spacing.x = t.space_3;
        let depth = indent * f32::from(row.depth.saturating_sub(self.base_depth)).min(6.0);
        let (label_rect, label_button) =
            child.allocate_exact_size(Vec2::new(label_width, t.size_control_inline), Sense::click());
        let label_name = middle(&label);
        label_button
            .widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, selected, &label_name));
        let mut job = egui::text::LayoutJob::single_section(
            label.clone(),
            egui::TextFormat::simple(crate::theme::font(&t.type_small), t.text_secondary.color32()),
        );
        job.wrap = egui::text::TextWrapping::truncate_at_width(label_width - depth);
        let galley = child.painter().layout_job(job);
        let elided = galley.elided;
        child.painter().galley(
            egui::pos2(
                label_rect.left() + depth,
                label_rect.center().y - galley.size().y / 2.0,
            ),
            galley,
            t.text_secondary.color32(),
        );
        let label_button = if elided {
            label_button.tip(&label)
        } else {
            label_button
        };
        if label_button.clicked() {
            events.push(Event::Action("ui.focus", json!({"area": "properties"})));
            events.push(Event::Action("props.row", json!({"to": row.key})));
        }
        let editing = props.edit().is_some_and(|e| e.row == row.key);
        // The chip's word: the source in one word, the whole text in the tooltip (doc 16a §6.8).
        let word = chip_word(&row.provenance);
        if editing {
            self.editor(&mut child, shell, row, &label, events);
        } else {
            // X32b (doc 16a §6.8, the mockup's `.prow`: `96px 1fr`): the value cell takes the rest of the row up to its
            // `space.4` padding. The row's "…" button sits over the cell's right end and only while it shows (the
            // cursor row, hover, focus) does the cell's text and chip make room for it.
            let actions_id = group_id.with("actions");
            let actions_on = !row.actions.is_empty()
                && (selected
                    || child.rect_contains_pointer(rect)
                    || child.memory(|m| m.has_focus(actions_id)));
            let reserve = if actions_on {
                t.size_control_inline + t.space_1
            } else {
                0.0
            };
            let value_w = child.available_width().max(t.size_control_inline);
            let value_rect;
            if let (FieldType::Bool, true) = (row.ty.inner(), row.actions.contains(&RowAction::Edit)) {
                // A checkbox (§6.5): a click sets the other value.
                let area =
                    egui::Rect::from_min_size(child.cursor().min, Vec2::new(value_w, t.size_control_inline));
                value_rect = area;
                let mut on = matches!(&row.value, RowValue::One { json, .. } if json.as_bool() == Some(true));
                let was = on;
                let response = child
                    .scope_builder(
                        egui::UiBuilder::new()
                            .max_rect(area)
                            .layout(egui::Layout::left_to_right(egui::Align::Center)),
                        |ui| ui.checkbox(&mut on, ""),
                    )
                    .inner;
                child.advance_cursor_after_rect(area);
                let name = label.clone();
                response.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, was, &name));
                if on != was {
                    events.push(Event::Action("ui.focus", json!({"area": "properties"})));
                    events.push(Event::Action(
                        "props.set",
                        json!({"row": row.key, "value": if on { "yes" } else { "no" }}),
                    ));
                }
                if show_chip {
                    let chip_rect = crate::components::paint_provenance_chip(
                        &child,
                        &t,
                        word,
                        tertiary,
                        egui::pos2(area.right() - t.space_icon - reserve, area.center().y),
                    );
                    child
                        .interact(chip_rect, group_id.with("chip"), Sense::hover())
                        .tip(&chip);
                }
            } else {
                // The value cell (the mockup's `.val`): a click edits in place; a transparent 1-px border that turns
                // `color.border.control` on hover; `space.icon` padding; an empty value is an em dash in
                // `color.text.tertiary`; the provenance chip at the cell's right end.
                let editable = !header && row.actions.contains(&RowAction::Edit);
                let (cell, response) = child.allocate_exact_size(
                    Vec2::new(value_w, t.size_control_inline),
                    if editable { Sense::click() } else { Sense::hover() },
                );
                let shown = if value.is_empty() && !header {
                    strings::text("props-empty-value").to_owned()
                } else {
                    middle(&value)
                };
                // The cell's node: the value (a button while a click edits it), inside the row's group.
                let cell_name = shown.clone();
                let role = if editable {
                    egui::Role::Button
                } else {
                    egui::Role::Label
                };
                response.widget_info(|| egui::WidgetInfo::labeled(role, true, &cell_name));
                if editable && response.hovered() {
                    child.painter().rect_stroke(
                        cell,
                        crate::theme::radius(t.radius_sm),
                        crate::theme::stroke(t.border_width, t.border_control),
                        egui::StrokeKind::Inside,
                    );
                }
                value_rect = cell;
                let mut text_right = cell.right() - t.space_icon - reserve;
                let mut on_chip = false;
                if show_chip {
                    let chip_rect = crate::components::paint_provenance_chip(
                        &child,
                        &t,
                        word,
                        tertiary,
                        egui::pos2(text_right, cell.center().y),
                    );
                    text_right = chip_rect.left() - t.space_2;
                    on_chip = response.hover_pos().is_some_and(|p| chip_rect.contains(p));
                }
                let colour = if value.is_empty() {
                    tertiary
                } else {
                    t.text_primary
                };
                // The value ellipsised at the chip (or the cell's end), the whole value in the tooltip.
                let mut job = egui::text::LayoutJob::single_section(
                    shown.clone(),
                    egui::TextFormat::simple(crate::theme::font(&t.type_body), colour.color32()),
                );
                job.wrap = egui::text::TextWrapping::truncate_at_width(
                    (text_right - cell.left() - t.space_icon).max(0.0),
                );
                let galley = child.painter().layout_job(job);
                let cut = galley.elided;
                child.painter().galley(
                    egui::pos2(
                        cell.left() + t.space_icon,
                        cell.center().y - galley.size().y / 2.0,
                    ),
                    galley,
                    colour.color32(),
                );
                // The tooltip: the chip's whole text over the chip, else a read-only value's reason, else a long
                // value's whole text.
                let long = (cut || value.chars().count() > MAX_SHOWN).then(|| value.clone());
                let tip = if on_chip {
                    Some(chip.clone())
                } else if !editable && !header {
                    reason.clone().or(long)
                } else {
                    long
                };
                let response = match tip {
                    Some(tip) => response.tip(tip),
                    None => response,
                };
                if response.clicked() {
                    events.push(Event::Action("ui.focus", json!({"area": "properties"})));
                    events.push(Event::Action("props.row", json!({"to": row.key})));
                    events.push(Event::Action("props.edit", json!({})));
                }
            }
            if !row.actions.is_empty() {
                // The actions menu: one "…" button on the cursor row, on hover and on focus.
                let id = actions_id;
                let r = egui::Rect::from_min_size(
                    egui::pos2(value_rect.right() - t.size_control_inline, value_rect.top()),
                    Vec2::splat(t.size_control_inline),
                );
                let name = strings::format("props-row-menu", &[&label]);
                let response = crate::command_bar::button_response(
                    &child,
                    id,
                    r,
                    crate::command_bar::Spec {
                        name: &name,
                        shortcut: "",
                        enabled: true,
                        checked: None,
                        sense: Sense::click(),
                    },
                );
                if actions_on || response.has_focus() {
                    crate::command_bar::state_overlay(&child, &t, &response, true, t.radius_sm);
                    let icon = egui::Rect::from_center_size(r.center(), Vec2::splat(t.icon_sm));
                    crate::command_bar::paint_icon(
                        &child,
                        "dots",
                        icon,
                        t.text_secondary.color32(),
                        t.icon_stroke,
                    );
                }
                let response = response.tip(&name);
                let open = matches!(
                    props.popup(),
                    Some(ew_viewmodel::properties::Popup::Actions { row: r, .. }) if *r == row.key
                );
                if response.clicked() {
                    events.push(Event::Action("ui.focus", json!({"area": "properties"})));
                    if open {
                        // A second click on "…" closes its menu.
                        events.push(Event::Action("props.cancel", json!({})));
                    } else {
                        events.push(Event::Action("props.row", json!({"to": row.key})));
                        events.push(Event::Action("props.row_menu", json!({})));
                    }
                } else if let Some(Popup::Actions { index, .. }) = props.popup().filter(|_| open) {
                    actions_menu(&t, &response, &name, &shell.row_menu(&row.key), *index, events);
                }
            }
        }
        // Below the row: its error (§6.1: the icon and the message in `color.error`, `type.small`), its choices, the
        // notes of the last commit, the popups.
        // M39: the message wraps inside the panel (never cut at its edge), its icon at the first line.
        if let Some(e) = error {
            ui.horizontal_top(|ui| {
                ui.add_space(t.space_4);
                let (r, _) = ui.allocate_exact_size(Vec2::splat(t.icon_sm), Sense::hover());
                crate::icons::paint(
                    ui.painter(),
                    crate::icons::IconId::circle_x,
                    r,
                    t.error.color32(),
                    t.icon_stroke,
                );
                ui.add(
                    egui::Label::new(
                        RichText::new(e)
                            .font(crate::theme::font(&t.type_small))
                            .color(t.error.color32()),
                    )
                    .wrap(),
                );
            });
            // FR-NUM-029: a refused tag-letter edit offers its proposal as one button (cycle 33, G-PP row 6).
            if let Some(p) = props.tag_proposal().filter(|p| p.row == row.key) {
                ui.horizontal(|ui| {
                    ui.add_space(t.space_4 + t.icon_sm);
                    // WP-S2.18 D3: "Join -K1" is a Secondary button (FT-628).
                    let clicked = match p.join() {
                        true => crate::components::button(
                            ui,
                            crate::components::Kind::Secondary,
                            &p.button_text(),
                            true,
                            0.0,
                        )
                        .clicked(),
                        false => ui.button(p.button_text()).clicked(),
                    };
                    if clicked {
                        events.push(Event::Action("props.accept_tag_proposal", json!({})));
                    }
                });
            }
        }
        if let Some(c) = props.choices().filter(|c| c.row == row.key) {
            self.choices(ui, &t, &label, &row.key, rect, c, events);
        } else if self.choices_rect.as_ref().is_some_and(|(r, _)| *r == row.key) {
            self.choices_rect = None;
        }
        match props.popup() {
            // FT-530: the actions menu is a popup anchored on the row's "…" button (`actions_menu`), not drawn here.
            Some(Popup::MlText { row: r }) if *r == row.key => {
                self.mltext_popup(ui, sheet, row, events);
            }
            _ => {
                self.mltext.clear();
            }
        }
        if selected && !props.notes().is_empty() {
            for n in props.notes() {
                ui.label(RichText::new(strings::format("props-notes", &[n])).small());
            }
        }
    }

    /// The options of a choice editor (a combo for enums and colours, a filtered picker for parts and structure nodes)
    /// and the readings of an ambiguous number: a list box in a popup over the grid, Up and Down move the highlight,
    /// Enter or a click chooses (§6.5, §6.9). The popup (doc 16a §6.22's look: `color.surface.raised`,
    /// `elevation.1`, `radius.md`, `space.2` padding, rows of `size.row`, at most `size.chooser` rows before it
    /// scrolls) opens under the row when it fits above the status bar, else over it, and is clamped to the window's
    /// free area (no part of it under the status bar or outside the window): `choices_placement`.
    #[allow(clippy::too_many_arguments)]
    fn choices(
        &mut self,
        ui: &mut Ui,
        t: &crate::theme::Tokens,
        label: &str,
        key: &str,
        anchor: egui::Rect,
        c: &ew_viewmodel::properties::Choices,
        events: &mut Vec<Event>,
    ) {
        let bound = ui
            .data(|d| d.get_temp::<egui::Rect>(free_area_id()))
            .unwrap_or_else(|| ui.ctx().content_rect());
        let width = anchor
            .width()
            .clamp(t.size_chooser_min_width, t.size_chooser_max_width);
        let rows = c.items.len().min(t.size_chooser_rows as usize).max(1);
        let content = t.size_row * rows as f32;
        let pad = t.space_2 + t.border_width;
        let (pos, max_height) = choices_placement(anchor, bound, width, content, pad, t.space_1);
        let list_id = ui.scope_id().with(("prop-choices", key));
        let area = egui::Area::new(list_id.with("area"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .constrain_to(bound)
            .show(ui.ctx(), |ui| {
                crate::components::card_frame(t)
                    .fill(t.surface_raised.color32())
                    .corner_radius(crate::theme::radius(t.radius_md))
                    .shadow(crate::theme::shadow(&t.elevation_1))
                    .inner_margin(egui::Margin::same(crate::theme::margin_px(t.space_2)))
                    .show(ui, |ui| {
                        ui.set_width(width - 2.0 * pad);
                        let list_rect = egui::Rect::from_min_size(
                            ui.max_rect().min,
                            Vec2::new(width - 2.0 * pad, max_height),
                        );
                        let list = ui.interact(list_rect, list_id, Sense::hover());
                        let list_name = strings::format("props-options", &[label]);
                        list.widget_info(|| egui::WidgetInfo::labeled(egui::Role::ListBox, true, &list_name));
                        ui.scope_builder(egui::UiBuilder::new().accessibility_parent(list_id), |ui| {
                            egui::ScrollArea::vertical()
                                .id_salt(("prop-choices-scroll", key))
                                .max_height(max_height)
                                .show(ui, |ui| {
                                    ui.spacing_mut().item_spacing.y = 0.0;
                                    for (i, (text, _)) in c.items.iter().enumerate() {
                                        let selected = c.index == Some(i);
                                        let (r, response) = ui.allocate_exact_size(
                                            Vec2::new(ui.available_width(), t.size_row),
                                            Sense::click(),
                                        );
                                        let name = text.clone();
                                        response.widget_info(|| {
                                            egui::WidgetInfo::selected(
                                                egui::Role::ListBoxOption,
                                                true,
                                                selected,
                                                &name,
                                            )
                                        });
                                        // The active row has the hover overlay (§6.22: as menus, never the accent).
                                        if selected || response.hovered() {
                                            ui.painter().rect_filled(
                                                r,
                                                crate::theme::radius(t.radius_sm),
                                                t.state_hover.color32(),
                                            );
                                        }
                                        ui.painter().text(
                                            egui::pos2(r.left() + t.space_2, r.center().y),
                                            egui::Align2::LEFT_CENTER,
                                            text,
                                            crate::theme::font(&t.type_body),
                                            t.text_primary.color32(),
                                        );
                                        if selected {
                                            response.scroll_to_me(None);
                                        }
                                        if response.clicked() {
                                            events.push(Event::Action("props.choose", json!({ "index": i })));
                                        }
                                    }
                                });
                        });
                    });
            });
        self.choices_rect = Some((key.to_owned(), area.response.rect));
    }

    /// The row's editor: a text input labelled by the field. Enter, Tab and leaving it commit; Escape reverts;
    /// Shift+Enter opens the multi-language popup.
    fn editor(
        &mut self,
        ui: &mut Ui,
        shell: &ShellModel,
        row: &PropRow,
        label: &str,
        events: &mut Vec<Event>,
    ) {
        let Some(edit) = shell.properties().edit() else {
            return;
        };
        let fresh = self.editing.as_deref() != Some(row.key.as_str());
        if fresh || (!ui.ctx().text_edit_focused() && self.buffer != edit.buffer) {
            self.buffer.clone_from(&edit.buffer);
            self.editing = Some(row.key.clone());
        }
        let shift_enter = ui.input(|i| i.modifiers.shift && i.key_pressed(Key::Enter));
        let escape = ui.input(|i| i.key_pressed(Key::Escape));
        // A choice editor's options: Up and Down move the highlight; Enter then chooses it.
        let choices = shell.properties().choices().filter(|c| c.row == row.key);
        let highlighted = choices.is_some_and(|c| c.index.is_some());
        if choices.is_some() {
            if ui
                .ctx()
                .input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown))
            {
                events.push(Event::Action("props.choose", json!({"step": 1})));
            } else if ui
                .ctx()
                .input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp))
            {
                events.push(Event::Action("props.choose", json!({"step": -1})));
            }
        }
        // Doc 16a §6.8 (the mockup's `.val.edit`): the value cell in its editing look, `color.surface.panel` with a
        // 1-px `color.accent` border and a 1-px accent ring, `radius.sm`, `space.icon` padding.
        let t = crate::theme::current(ui.ctx());
        let background = ui.painter().add(egui::Shape::Noop);
        // Cycle 37 (window check): an editor opened on the stored value (Enter, F2, Edit) starts with the whole text
        // selected, so typing replaces it ("-101K2" then "-101K1" gave "-101K2-101K1"); one opened by typing keeps
        // its caret after the typed text.
        // The selection is set once the field holds the focus (egui collapses a selection of an unfocused field).
        let field_id = ui.make_persistent_id(("props-editor", row.key.as_str()));
        let response = ui.add_sized(
            Vec2::new(ui.available_width(), t.size_control_inline),
            TextEdit::singleline(&mut self.buffer)
                .id(field_id)
                .frame(egui::Frame::NONE)
                .margin(egui::Margin::symmetric(crate::theme::margin_px(t.space_icon), 0))
                .desired_width(f32::INFINITY)
                .hint_text(label),
        );
        let radius = crate::theme::radius(t.radius_sm);
        ui.painter().set(
            background,
            egui::epaint::RectShape::filled(response.rect, radius, t.surface_panel.color32()),
        );
        let accent = crate::theme::stroke(t.border_width, t.accent);
        ui.painter()
            .rect_stroke(response.rect, radius, accent, egui::StrokeKind::Inside);
        ui.painter()
            .rect_stroke(response.rect, radius, accent, egui::StrokeKind::Outside);
        crate::focus::report_focus(
            &response,
            shell.focus() == Some(Focus::Properties),
            "properties",
            events,
        );
        let name = label.to_owned();
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, &name));
        if fresh {
            response.request_focus();
        }
        if response.gained_focus() && edit.initial.as_deref() == Some(self.buffer.as_str()) {
            let mut state = egui::text_edit::TextEditState::load(ui.ctx(), field_id).unwrap_or_default();
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::select_all_str(&self.buffer)));
            state.store(ui.ctx(), field_id);
            ui.ctx().request_repaint();
        }
        if response.changed() {
            events.push(Event::Action("props.input", json!({ "text": self.buffer })));
        }
        // A pointer press on the options takes the field's focus: the click chooses, the focus loss commits nothing.
        let on_options = ui.input(|i| i.pointer.interact_pos()).is_some_and(|p| {
            self.choices_rect
                .as_ref()
                .is_some_and(|(r, rect)| *r == row.key && rect.contains(p))
        });
        if response.lost_focus() && on_options && !escape {
            return;
        }
        if response.lost_focus() {
            self.editing = None;
            if escape {
                events.push(Event::Action("props.cancel", json!({})));
            } else if shift_enter {
                events.push(Event::Action("props.mltext", json!({"row": row.key})));
            } else if highlighted {
                events.push(Event::Action("props.choose", json!({})));
            } else {
                events.push(Event::Action("props.commit", json!({})));
            }
        }
    }

    /// The multi-language popup (§6.5): a "Neutral" row and one row per project language, source first; Enter in a
    /// row sets that text.
    fn mltext_popup(&mut self, ui: &mut Ui, sheet: &PropertySheet, row: &PropRow, events: &mut Vec<Event>) {
        if self.mltext.is_empty() {
            let current = match &row.value {
                RowValue::One { json, .. } => json.clone(),
                _ => serde_json::Value::Null,
            };
            self.mltext
                .push((None, current.as_str().unwrap_or_default().to_owned()));
            for lang in &sheet.languages {
                let text = current
                    .get(lang)
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_owned();
                self.mltext.push((Some(lang.clone()), text));
            }
        }
        for (lang, text) in &mut self.mltext {
            ui.horizontal(|ui| {
                let name = lang
                    .clone()
                    .unwrap_or_else(|| strings::text("props-neutral").to_owned());
                ui.label(&name);
                let response = ui.add(TextEdit::singleline(text).hint_text(name.clone()));
                response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, &name));
                if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    let mut params = json!({"row": row.key, "text": text});
                    if let Some(l) = lang {
                        params["lang"] = json!(l);
                    }
                    events.push(Event::Action("props.mltext", params));
                }
            });
        }
    }
}

/// The key of the window's free area in the context's memory.
fn free_area_id() -> egui::Id {
    egui::Id::NULL.with("properties-free-area")
}

/// Remembers `ui`'s rectangle, the central panel's (the window less the command bar, the rails and the status bar),
/// as the area a property popup must stay inside this frame.
pub(crate) fn remember_free_area(ui: &Ui) {
    let rect = ui.max_rect();
    ui.data_mut(|d| d.insert_temp(free_area_id(), rect));
}

/// Where the options popup of a choice editor opens (the top-left corner) and how high its list may be before it
/// scrolls: under `anchor` (the row) when the list's `content` height and its frame (`pad` on each side) fit between
/// the row and `bound`'s bottom (the status bar), else over the row when they fit above it, else on the side with
/// more room with the list cut to that room. `gap` separates the popup from the row. The popup is `width` wide, its
/// left edge at the row's, moved left as far as needed to stay inside `bound`.
pub(crate) fn choices_placement(
    anchor: egui::Rect,
    bound: egui::Rect,
    width: f32,
    content: f32,
    pad: f32,
    gap: f32,
) -> (egui::Pos2, f32) {
    let full = content + 2.0 * pad;
    let below = bound.bottom() - (anchor.bottom() + gap);
    let above = (anchor.top() - gap) - bound.top();
    let x = anchor.left().min(bound.right() - width).max(bound.left());
    if full <= below {
        (egui::pos2(x, anchor.bottom() + gap), content)
    } else if full <= above {
        (egui::pos2(x, anchor.top() - gap - full), content)
    } else if below >= above {
        (egui::pos2(x, anchor.bottom() + gap), (below - 2.0 * pad).max(0.0))
    } else {
        (egui::pos2(x, bound.top()), (above - 2.0 * pad).max(0.0))
    }
}

/// The focused row's key and value when it is an editable boolean (Space toggles it).
fn bool_row(shell: &ShellModel, key: &str) -> Option<(String, bool)> {
    let sheet = shell.properties().sheet()?;
    let (_, row) = ew_viewmodel::properties::find_row(sheet, key)?;
    if !matches!(row.ty.inner(), FieldType::Bool) || !row.actions.contains(&RowAction::Edit) {
        return None;
    }
    let on = matches!(&row.value, RowValue::One { json, .. } if json.as_bool() == Some(true));
    Some((row.key.clone(), on))
}

/// Whether `s` is one of the grid's own keys (the arrows, Home, End, Enter, Insert, Delete), which a global chord must
/// not take while the grid has the focus.
pub fn is_grid_key(s: &egui::KeyboardShortcut) -> bool {
    let plain = s.modifiers == Modifiers::NONE;
    let alt_arrow = s.modifiers == Modifiers::ALT && matches!(s.logical_key, Key::ArrowUp | Key::ArrowDown);
    let grid = matches!(
        s.logical_key,
        Key::ArrowUp
            | Key::ArrowDown
            | Key::ArrowLeft
            | Key::ArrowRight
            | Key::Home
            | Key::End
            | Key::Enter
            | Key::Insert
            | Key::Delete
            | Key::Space
    );
    (plain && grid) || alt_arrow
}

/// The row's actions menu (FT-530; doc 16a §6.8, §6.13): a popup menu anchored on the row's "…" button (`anchor`),
/// named `name`; one row of `size.row` per action with an empty 16-px icon column and its label, the highlighted one
/// (`index`, Up and Down: `props.row_menu`) and the hovered one under the hover overlay, never the accent. A click
/// chooses (`props.choose`); a click elsewhere or Escape closes it (`props.cancel`).
fn actions_menu(
    t: &crate::theme::Tokens,
    anchor: &egui::Response,
    name: &str,
    items: &[ew_viewmodel::properties::RowMenuItem],
    index: usize,
    events: &mut Vec<Event>,
) {
    let config = crate::menu::menu_config(t);
    let mut chose = false;
    let shown = egui::Popup::menu(anchor)
        .open(true)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .style(config.style.clone())
        .accessible_name(name)
        .width(t.size_chooser_min_width)
        .show(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for (i, a) in items.iter().enumerate() {
                let label = ew_viewmodel::properties::menu_label(*a);
                let (r, response) =
                    ui.allocate_exact_size(Vec2::new(ui.available_width(), t.size_row), Sense::click());
                response.widget_info(|| {
                    egui::WidgetInfo::selected(egui::Role::MenuItem, true, i == index, label)
                });
                if i == index || response.hovered() {
                    ui.painter()
                        .rect_filled(r, crate::theme::radius(t.radius_sm), t.state_hover.color32());
                }
                ui.painter().text(
                    egui::pos2(r.left() + t.space_2 + t.icon_sm + t.space_2, r.center().y),
                    egui::Align2::LEFT_CENTER,
                    label,
                    crate::theme::font(&t.type_body),
                    t.text_primary.color32(),
                );
                if response.clicked() {
                    chose = true;
                    events.push(Event::Action("props.choose", json!({ "index": i })));
                }
            }
        });
    if !chose && shown.is_some_and(|s| s.response.should_close()) {
        events.push(Event::Action("props.cancel", json!({})));
    }
}

/// A group's heading (doc 16a §6.8, the mockup's `.psec`): `size.row` high with `space.4` side padding, a chevron and
/// the title in `type.small` SemiBold `color.text.secondary`, as written ("Main", not upper case); a button that
/// collapses or expands the group; `true` when clicked.
fn group_heading(ui: &mut Ui, t: &crate::theme::Tokens, title: &str, key: &str, shut: bool) -> bool {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), t.size_row), Sense::hover());
    let id = ui.scope_id().with(("prop-group", key));
    let response = ui.interact(rect, id, Sense::click());
    let name = title.to_owned();
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, !shut, &name));
    crate::command_bar::state_overlay(ui, t, &response, true, t.radius_sm);
    let chevron = if shut { "chevron-right" } else { "chevron-down" };
    let icon = egui::Rect::from_min_size(
        egui::pos2(rect.left() + t.space_4, rect.center().y - t.icon_sm / 2.0),
        Vec2::splat(t.icon_sm),
    );
    crate::command_bar::paint_icon(ui, chevron, icon, t.text_secondary.color32(), t.icon_stroke);
    let font = crate::theme::font(&crate::theme::TypeToken {
        semibold: true,
        ..t.type_small
    });
    ui.painter().text(
        egui::pos2(icon.right() + t.space_icon, rect.center().y),
        egui::Align2::LEFT_CENTER,
        title,
        font,
        t.text_secondary.color32(),
    );
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect,
            crate::theme::radius(t.radius_sm),
            crate::theme::stroke(t.focus_width, t.focus),
            egui::StrokeKind::Inside,
        );
    }
    response.clicked()
}

/// The header's findings by severity, most severe first (`error`, `warning`, then every other severity as `info`),
/// then `pending`, each with its `(rule, message)` pairs; severities without a finding are left out. M39: a finding
/// pending while `editing` (the editing page's folio) is the editing page counts as pending, not under its severity,
/// as the Messages header counts it.
fn severity_groups<'a>(
    findings: &'a [ew_viewmodel::ew_api::output::FindingChip],
    editing: Option<&str>,
) -> Vec<(&'static str, Vec<(&'a str, &'a str)>)> {
    let pending = |f: &ew_viewmodel::ew_api::output::FindingChip| {
        ew_viewmodel::navigators::messages::chip_pending(f, editing)
    };
    ["error", "warning", "info", "pending"]
        .into_iter()
        .map(|severity| {
            let of: Vec<(&str, &str)> = findings
                .iter()
                .filter(|f| match severity {
                    "pending" => pending(f),
                    _ if pending(f) => false,
                    "error" | "warning" => f.severity == severity,
                    _ => f.severity != "error" && f.severity != "warning",
                })
                .map(|f| (f.rule.as_str(), f.message.as_str()))
                .collect();
            (severity, of)
        })
        .filter(|(_, of)| !of.is_empty())
        .collect()
}

/// "<n> <Word(s)>" ("1 Warning", "3 Warnings"), a count chip's word.
fn count_words(severity: &str, n: usize) -> String {
    crate::messages::count_words(severity, n)
}

/// The Properties label column's width in a row `row` wide whose widest label on show needs `need`: `need`, never
/// below `size.property_label` and never above 40 % of the row (the floor wins in a row too narrow for both).
pub(crate) fn label_column(t: &crate::theme::Tokens, row: f32, need: f32) -> f32 {
    need.min(row * 0.4).max(t.size_property_label)
}

/// Whether `row` is a page's Kind (its page type, a raw word like `multi_line` that the grid shows in words).
fn is_page_kind(section: &ew_viewmodel::ew_api::output::SheetSection, row: &PropRow) -> bool {
    section.kind == "page" && (row.key == "page/kind" || row.key == "page/type")
}

/// A severity's glyph chip (FT-664): the severity's subtle fill, its icon, and the count in `type.caption` when more
/// than one; named like the count chip ("2 Warnings: <rule>: <message>; …"), every finding in its tooltip.
/// The actions of a click on a Properties finding chip (FT-759): Messages opens, filtered to the record
/// ([`findings_filter`]).
fn findings_events(header: &ew_viewmodel::ew_api::output::SheetHeader) -> [Event; 2] {
    [
        Event::Action("messages.show", json!({"on": true})),
        Event::Action(
            "navigator.filter",
            json!({"kind": "messages", "text": findings_filter(header)}),
        ),
    ]
}

/// The Messages filter of a sheet's record: a page by its folio facet (`page:101`; its title "101 Control" is in no
/// finding's text), any other record by its title (a device's tag, a wire's number).
pub(crate) fn findings_filter(header: &ew_viewmodel::ew_api::output::SheetHeader) -> String {
    let page = header.items == 1 && header.kinds.iter().any(|(k, n)| k == "page" && *n == 1);
    match header.title.split_whitespace().next() {
        Some(folio) if page => format!("page:{folio}"),
        _ => header.title.clone(),
    }
}

fn glyph_chip(ui: &mut Ui, severity: &str, findings: &[(&str, &str)]) -> egui::Response {
    let t = crate::theme::current(ui.ctx());
    let (fill, text, icon_colour) = match severity {
        "error" => (t.error_subtle, t.error, t.error),
        "warning" => (t.warning_subtle, t.warning, t.warning_icon),
        "pending" => (t.surface_sunken, t.text_secondary, t.text_secondary),
        _ => (t.info_subtle, t.info, t.info),
    };
    let count = (findings.len() > 1).then(|| {
        ui.painter().layout_no_wrap(
            findings.len().to_string(),
            crate::theme::font(&t.type_caption),
            text.color32(),
        )
    });
    let icon = t.type_caption.size + t.space_1;
    let width = t.space_1 + icon + count.as_ref().map_or(0.0, |g| t.space_1 + g.size().x) + t.space_1;
    // FT-759: a button that opens Messages filtered to the record.
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, t.size_badge), Sense::click());
    let lines: Vec<String> = findings
        .iter()
        .map(|(rule, message)| strings::format("props-finding-tip", &[rule, message]))
        .collect();
    let name = strings::format(
        "props-findings-chip",
        &[&count_words(severity, findings.len()), &lines.join("; ")],
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &name));
    let painter = ui.painter();
    painter.rect_filled(rect, crate::theme::radius(t.radius_sm), fill.color32());
    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(rect.left() + t.space_1 + icon / 2.0, rect.center().y),
        Vec2::splat(icon),
    );
    crate::icons::paint(
        painter,
        crate::messages::severity_icon_id(severity),
        icon_rect,
        icon_colour.color32(),
        t.icon_stroke,
    );
    if let Some(g) = count {
        painter.galley(
            egui::pos2(icon_rect.right() + t.space_1, rect.center().y - g.size().y / 2.0),
            g,
            text.color32(),
        );
    }
    response.tip(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use egui::{Rect, pos2};

    use super::choices_placement;

    /// M39: the label column scales with the row (40 %), never under `size.property_label`.
    #[test]
    fn the_label_column_scales_with_the_panel() {
        use crate::theme::{Density, Theme, tokens};
        let t = tokens(Theme::Light, Density::Comfortable);
        // Short labels: the floor. A long label: its width, up to 40 % of the row.
        assert_eq!(super::label_column(&t, 600.0, 40.0), t.size_property_label);
        assert_eq!(super::label_column(&t, 600.0, 150.0), 150.0);
        assert_eq!(super::label_column(&t, 600.0, 400.0), 240.0);
        assert_eq!(super::label_column(&t, 100.0, 400.0), t.size_property_label);
    }

    /// M39: a finding pending on the editing page counts as pending, not under its severity; elsewhere it counts
    /// under its severity, as the Messages header counts it.
    #[test]
    fn pending_findings_count_as_pending() {
        use ew_viewmodel::ew_api::output::FindingChip;
        let chip = |severity: &str, pending: Option<&str>| FindingChip {
            rule: "EW-CORE-041".into(),
            severity: severity.into(),
            message: "m".into(),
            pending_on: pending.map(str::to_owned),
        };
        let findings = [
            chip("warning", Some("101")),
            chip("warning", None),
            chip("error", Some("102")),
        ];
        let groups = super::severity_groups(&findings, Some("101"));
        let count = |s: &str| groups.iter().find(|(g, _)| *g == s).map_or(0, |(_, of)| of.len());
        assert_eq!((count("error"), count("warning"), count("pending")), (1, 1, 1));
        let none = super::severity_groups(&findings, None);
        assert!(none.iter().all(|(g, _)| *g != "pending"));
    }

    /// FT-759: a page's chip filters Messages by its folio facet, a device's by its tag.
    #[test]
    fn a_finding_chip_filters_a_page_by_folio_and_a_device_by_tag() {
        use ew_viewmodel::ew_api::output::SheetHeader;
        let page = SheetHeader {
            title: "101 Control".into(),
            items: 1,
            kinds: vec![("page".into(), 1)],
            ..SheetHeader::default()
        };
        assert_eq!(super::findings_filter(&page), "page:101");
        let device = SheetHeader {
            title: "-101S1".into(),
            items: 1,
            kinds: vec![("device".into(), 1)],
            ..SheetHeader::default()
        };
        assert_eq!(super::findings_filter(&device), "-101S1");
    }

    /// The free area of a 1280 by 800 window: above the status bar, between the rails.
    fn bound() -> Rect {
        Rect::from_min_max(pos2(40.0, 117.0), pos2(1240.0, 774.0))
    }

    #[test]
    fn the_options_open_under_the_row_when_they_fit() {
        let row = Rect::from_min_max(pos2(60.0, 300.0), pos2(320.0, 328.0));
        let (at, h) = choices_placement(row, bound(), 260.0, 224.0, 9.0, 4.0);
        assert_eq!(at, pos2(60.0, 332.0));
        assert_eq!(h, 224.0);
    }

    #[test]
    fn the_options_open_over_the_row_near_the_status_bar() {
        let row = Rect::from_min_max(pos2(60.0, 600.0), pos2(320.0, 628.0));
        let (at, h) = choices_placement(row, bound(), 260.0, 224.0, 9.0, 4.0);
        // The list and its frame (224 + 2 × 9) end 4 px over the row.
        assert_eq!(at, pos2(60.0, 600.0 - 4.0 - 242.0));
        assert_eq!(h, 224.0);
    }

    #[test]
    fn a_list_taller_than_both_sides_takes_the_larger_side_and_scrolls() {
        let bound = Rect::from_min_max(pos2(0.0, 100.0), pos2(400.0, 400.0));
        // Above: 196 px, below: 68 px; the popup starts at the bound's top, its list 196 - 18 high.
        let row = Rect::from_min_max(pos2(10.0, 300.0), pos2(200.0, 328.0));
        let (at, h) = choices_placement(row, bound, 260.0, 500.0, 9.0, 4.0);
        assert_eq!(at, pos2(10.0, 100.0));
        assert_eq!(h, 178.0);
        // Above: 46 px, below: 218 px; under the row, its list 218 - 18 high.
        let row = Rect::from_min_max(pos2(10.0, 150.0), pos2(200.0, 178.0));
        let (at, h) = choices_placement(row, bound, 260.0, 500.0, 9.0, 4.0);
        assert_eq!(at, pos2(10.0, 182.0));
        assert_eq!(h, 200.0);
    }

    #[test]
    fn the_options_stay_inside_the_window_horizontally() {
        let row = Rect::from_min_max(pos2(1100.0, 300.0), pos2(1230.0, 328.0));
        let (at, _) = choices_placement(row, bound(), 260.0, 224.0, 9.0, 4.0);
        assert_eq!(at.x, 1240.0 - 260.0);
    }
}
