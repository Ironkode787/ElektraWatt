// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The command palette (WP-S2.01 part 1, spec section 5.3; FR-UI-012 part-1 subset). Ctrl+K opens a popup with a
//! text field over all registry actions and the registered model commands; matching is a case-insensitive
//! substring on the label and the ID; disabled entries are listed last with their reason; Enter runs the selected
//! enabled entry; Escape closes it. Ranking, aliases, prefixes and parameter forms are WP-S2.08.

use egui::{Align2, Button, Id, Key, Modifiers, ScrollArea, TextEdit, Window};
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;

use crate::app::Event;
use crate::keys;

/// Paints the palette when it is open. `just_opened` puts the keyboard focus into its text field. `anchor` is the
/// command bar's search field (WP-S2.17 part 2, doc 16a §5.2): the palette opens under it, right-aligned with it.
pub fn show(
    ctx: &egui::Context,
    shell: &ShellModel,
    just_opened: bool,
    anchor: Option<egui::Rect>,
    events: &mut Vec<Event>,
) {
    let Some(state) = shell.palette() else {
        return;
    };
    // The keys of the list are consumed before the text field sees them.
    let (down, up, enter) = ctx.input_mut(|i| {
        (
            i.consume_key(Modifiers::NONE, Key::ArrowDown),
            i.consume_key(Modifiers::NONE, Key::ArrowUp),
            i.consume_key(Modifiers::NONE, Key::Enter),
        )
    });
    if down {
        events.push(Event::PaletteMove(1));
    }
    if up {
        events.push(Event::PaletteMove(-1));
    }
    if enter {
        events.push(Event::PaletteRun);
    }
    let entries = shell.palette_entries();
    let window = Window::new(strings::text("palette-title")).id(Id::unique("command-palette"));
    let gap = crate::theme::current(ctx).space_1;
    let window = match anchor {
        Some(field) => window
            .pivot(Align2::RIGHT_TOP)
            .fixed_pos(field.right_bottom() + egui::vec2(0.0, gap)),
        None => window.anchor(Align2::CENTER_TOP, [0.0, 60.0]),
    };
    window
        .collapsible(false)
        .resizable(false)
        .default_width(480.0)
        .show(ctx, |ui| {
            let mut query = state.query.clone();
            let edit = ui.add(
                TextEdit::singleline(&mut query)
                    .hint_text(strings::text("palette-hint"))
                    .desired_width(f32::INFINITY),
            );
            if just_opened {
                edit.request_focus();
            }
            if query != state.query {
                events.push(Event::PaletteQuery(query));
            }
            ui.separator();
            if entries.is_empty() {
                ui.label(strings::text("palette-no-match"));
            }
            ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                for (i, entry) in entries.iter().enumerate() {
                    ui.horizontal(|ui| {
                        let mut button = Button::new(entry.label.as_str()).selected(i == state.selected);
                        if !entry.shortcut.is_empty() {
                            button = button.shortcut_text(keys::display(&entry.shortcut));
                        }
                        if ui.add_enabled(entry.enabled, button).clicked() {
                            events.push(Event::PaletteClick(i));
                        }
                        if let Some(reason) = &entry.reason {
                            ui.weak(reason.as_str());
                        }
                    });
                }
            });
        });
}
