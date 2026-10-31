// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The junction sequence editor popup (WP-S2.04 §8.5, FR-CON-005): the junction's style and wiring as buttons, its
//! sequence as rows named by each member's far endpoint, the physical wires through it below. Up and Down focus a row
//! (a view change of the popup), Alt+Up and Alt+Down move the focused member (`junction.member`), R cycles a bevel's
//! orientation (`junction.style`), Escape closes it (`ui.dismiss`, the topmost popup). Every change is a registry
//! action, so the headless mode drives the same model; each commits one `junction.edit`.

use egui::{Align2, Button, Id, Key, Modifiers, Window};
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;
use serde_json::json;

use crate::app::Event;

/// Paints the editor when it is open.
pub fn show(ctx: &egui::Context, shell: &ShellModel, events: &mut Vec<Event>) {
    let Some(editor) = shell.junction_editor() else {
        return;
    };
    let n = editor.members.len();
    let row = editor.row;
    let (alt_up, alt_down, up, down, rotate) = ctx.input_mut(|i| {
        (
            i.consume_key(Modifiers::ALT, Key::ArrowUp),
            i.consume_key(Modifiers::ALT, Key::ArrowDown),
            i.consume_key(Modifiers::NONE, Key::ArrowUp),
            i.consume_key(Modifiers::NONE, Key::ArrowDown),
            i.consume_key(Modifiers::NONE, Key::R),
        )
    });
    if alt_up && row > 0 {
        events.push(Event::Action(
            "junction.member",
            json!({"from": row, "to": row - 1}),
        ));
    } else if alt_down && row + 1 < n {
        events.push(Event::Action(
            "junction.member",
            json!({"from": row, "to": row + 1}),
        ));
    } else if up && row > 0 {
        events.push(Event::JunctionRow(row - 1));
    } else if down && row + 1 < n {
        events.push(Event::JunctionRow(row + 1));
    }
    if rotate && editor.style == "bevel" {
        let next = (editor.bevel_orient & 4) | ((editor.bevel_orient + 1) & 3);
        events.push(Event::Action(
            "junction.style",
            json!({"style": "bevel", "orient": next}),
        ));
    }
    Window::new(strings::format("junction-title", &[&editor.cell]))
        .id(Id::unique("junction-editor"))
        .anchor(Align2::CENTER_TOP, [0.0, 60.0])
        .collapsible(false)
        .resizable(false)
        // C40 (FT-816): kept at `size.dialog.small` on purpose: two button rows and a short member list, which a
        // wider window would only pad with empty space.
        .default_width(crate::theme::current(ctx).size_dialog_small)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(strings::text("junction-style"));
                for (style, key) in [("dot", "junction-style-dot"), ("bevel", "junction-style-bevel")] {
                    if ui
                        .add(Button::new(strings::text(key)).selected(editor.style == style))
                        .clicked()
                    {
                        events.push(Event::Action("junction.style", json!({ "style": style })));
                    }
                }
                if editor.style == "bevel" {
                    ui.weak(strings::format(
                        "junction-orient",
                        &[&ew_viewmodel::ew_api::geometry::variant_letter(editor.bevel_orient).to_string()],
                    ));
                }
            });
            ui.horizontal(|ui| {
                ui.label(strings::text("junction-wiring"));
                for (wiring, key, selected) in [
                    (
                        "star",
                        "junction-wiring-star",
                        editor.explicit_wiring && editor.wiring == "star",
                    ),
                    (
                        "chain",
                        "junction-wiring-chain",
                        editor.explicit_wiring && editor.wiring == "chain",
                    ),
                    (
                        "style_default",
                        "junction-wiring-default",
                        !editor.explicit_wiring,
                    ),
                ] {
                    if ui
                        .add(Button::new(strings::text(key)).selected(selected))
                        .clicked()
                    {
                        events.push(Event::Action("junction.wiring", json!({ "wiring": wiring })));
                    }
                }
            });
            ui.separator();
            ui.label(strings::text("junction-sequence"));
            for (i, m) in editor.members.iter().enumerate() {
                if ui
                    .add(Button::new(format!("{}  {}", i + 1, m.label)).selected(i == row))
                    .clicked()
                {
                    events.push(Event::JunctionRow(i));
                }
            }
            ui.separator();
            ui.label(strings::text("junction-wires"));
            for w in &editor.wires {
                ui.label(strings::format("junction-wire-row", &[&w.label, &w.from, &w.to]));
            }
        });
}
