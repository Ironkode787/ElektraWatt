// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The routing of the keyboard: what lies over the window, the panes' own keys, the command bar's arrows, the chords
//! by physical and by logical key, the clipboard events, and the app menu's Alt and F10.

use super::*;

impl App {
    /// The shell's shortcuts. Routing order: a focused text field, then an open popup or dialog, then the
    /// shortcuts; Escape closes the topmost popup or dialog and otherwise goes to the active canvas tool through
    /// `ui.dismiss`. Help's keys are handled at the end of the frame, in every state. While a canvas has the
    /// focus, `[` and `]` are matched as the physical keys (no AltGr on QWERTZ and AZERTY, doc 16 §4.4).
    pub(super) fn route_keys(&mut self, ctx: &egui::Context, events: &mut Vec<Event>) {
        // The platform answers the last frame's request for the clipboard in this frame, or not at all.
        let pasting = self.pasting.take();
        let palette_open = self.shell.palette().is_some();
        if self.popup_keys(ctx, palette_open, events) || self.pane_keys(ctx, palette_open, events) {
            return;
        }
        let focused = self.key_context();
        if !palette_open
            && (self.command_bar_keys(ctx, events)
                || self.physical_keys(ctx, focused, events)
                || self.clipboard_keys(ctx, focused, pasting, events))
        {
            return;
        }
        self.bound_keys(ctx, focused, palette_open, events);
    }

    /// The keys of what lies over the window, in order: Enter in the supply marks' panel, Escape for an open menu,
    /// the collapsed bar's popover, the palette, a dialog or a popup, and the start-up offer; then nothing more while a
    /// dialog or a popup is open or a text field outside the palette has the focus. `true` when the routing ends here.
    fn popup_keys(&mut self, ctx: &egui::Context, palette_open: bool, events: &mut Vec<Event>) -> bool {
        let dialog_open = self.shell.dialog().is_some();
        // The junction sequence editor is a popup like the picker: Escape closes it, its own keys
        // stay with it. The text popup is one too.
        let picker_open = self.shell.picker().is_some()
            // The Join chooser is a popup too.
            || self.shell.chooser().is_some()
            || self.shell.junction_editor().is_some()
            || self.shell.text_popup().is_some()
            // The notifications log is a popup too.
            || self.shell.notifications().popup_open();
        let overlay = palette_open || dialog_open || picker_open;
        // Enter in the supply marks' Potential panel takes every row (H9), before the canvas's Enter.
        if self.shell.supply_panel().is_some()
            && !overlay
            && !egui::Popup::is_any_open(ctx)
            && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter))
        {
            events.push(Event::Action("ip.supply_potential", json!({"apply": true})));
            return true;
        }
        // Escape closes the app menu (with its submenus), then the collapsed bar's command-row popover.
        // Any open menu, a context menu among them, closes and nothing else (the selection stays).
        if egui::Popup::is_any_open(ctx) && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            egui::Popup::close_all(ctx);
            return true;
        }
        if self.command_bar.popover_open()
            && !overlay
            && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape))
        {
            self.command_bar.close_popover();
            return true;
        }
        if overlay && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            events.push(Event::Action("ui.dismiss", Value::Null));
            return true;
        }
        // The start-up offer is modal; Escape is its Not now.
        if self.shell.startup_offer().is_some() {
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
                events.push(Event::Action("startup.dismiss", Value::Null));
            }
            return true;
        }
        dialog_open || picker_open || (ctx.text_edit_focused() && !palette_open)
    }

    /// The keys a pane keeps before the shortcuts: Escape with a Properties editor or a station's tag field open
    /// (Enter too for the tag), Escape as `ui.dismiss`, the history panel's row keys, and Apply's keys for an open
    /// parts import card. `true` when the routing ends here.
    fn pane_keys(&mut self, ctx: &egui::Context, palette_open: bool, events: &mut Vec<Event>) -> bool {
        // Escape with a Properties editor open is the editor's (`props.cancel`), whether or not its field
        // still holds the focus: egui drops the focus on Escape before the frame, so `ui.dismiss` took it and the field
        // then committed on the lost focus (refused: nothing was being edited).
        if self.properties.field_open() && ctx.input(|i| i.key_pressed(Key::Escape)) {
            return true;
        }
        // So are Enter and Escape with a station view's tag field open: they take or leave the tag.
        if self.shell.station_tab().is_some_and(|t| t.rename.is_some())
            && ctx.input(|i| i.key_pressed(Key::Escape) || i.key_pressed(Key::Enter))
        {
            return true;
        }
        // So is Escape in a field of the tool options card: it leaves the field without committing (as egui drops the
        // focus before the frame, by the field the last frame left typing), and the tool stays.
        if self.options_typing && ctx.input(|i| i.key_pressed(Key::Escape)) {
            return true;
        }
        // While the symbol editor shows, Escape is its own (a gesture ends, then Select).
        if !palette_open
            && self.shell.has_project()
            && self.shell.symbol_editor().is_none()
            && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape))
        {
            events.push(Event::Action("ui.dismiss", Value::Null));
            return true;
        }
        // The history panel's row keys while it has the focus.
        if !palette_open && crate::history::keys(ctx, events) {
            return true;
        }
        // The keys of `preview.apply` (Ctrl+Enter) apply an open parts import card from anywhere (the card is a window
        // of its own).
        if !palette_open
            && self.shell.import_card_open()
            && (self.bindings.iter())
                .filter(|b| b.action == "preview.apply")
                .any(|b| ctx.input_mut(|i| i.consume_shortcut(&b.shortcut)))
        {
            events.push(Event::Action("preview.apply", Value::Null));
            return true;
        }
        false
    }

    /// The keymap's context of the focus: the focused context's chords go first, then the global ones.
    fn key_context(&self) -> Context {
        match self.shell.focus() {
            // Doc 16a P8: the symbol editor's drawing is a canvas; its tools take the canvas's keys.
            _ if self.shell.symbol_editor().is_some() => Context::Canvas,
            Some(Focus::Navigator(_)) => Context::Navigator,
            Some(Focus::Canvas(_)) => Context::Canvas,
            Some(Focus::Properties) => Context::Properties,
            Some(Focus::Find) => Context::Find,
            Some(Focus::CommandBar | Focus::History) | None => Context::Global,
        }
    }

    /// Doc 16a §5.9: on the command bar Left and Right move between the tabs. `true` when one did.
    fn command_bar_keys(&mut self, ctx: &egui::Context, events: &mut Vec<Event>) -> bool {
        if self.shell.focus() != Some(Focus::CommandBar) {
            return false;
        }
        let step = ctx.input_mut(|i| {
            if i.consume_key(Modifiers::NONE, Key::ArrowLeft) {
                -1
            } else if i.consume_key(Modifiers::NONE, Key::ArrowRight) {
                1
            } else {
                0
            }
        });
        if step == 0 {
            return false;
        }
        let tab = crate::command_bar::step_tab(self.shell.command_tab(), step);
        events.push(Event::Action("view.command_tab", json!({ "tab": tab.word() })));
        ctx.memory_mut(|m| m.request_focus(crate::command_bar::tab_id(tab)));
        true
    }

    /// The chords bound by physical key (`]` and `[` in the canvas, any `phys:` key of a keymap), matched on the key's
    /// position, so they need no AltGr on QWERTZ and AZERTY; a chord without Shift also takes the key with Shift.
    /// `true` when one matched.
    fn physical_keys(&mut self, ctx: &egui::Context, focused: Context, events: &mut Vec<Event>) -> bool {
        let physical = candidates(
            &self.bindings,
            self.shell.symbol_editor().is_some(),
            true,
            focused,
        );
        let hit = ctx.input_mut(|i| {
            let mut hit = None;
            i.events.retain(|e| match e {
                // A platform that reports no physical key (a test's) has its logical key matched instead.
                egui::Event::Key {
                    key,
                    physical_key,
                    pressed: true,
                    modifiers,
                    ..
                } if hit.is_none() => {
                    let k = physical_key.unwrap_or(*key);
                    let found = physical.iter().find(|b| {
                        let want = b.shortcut.modifiers;
                        b.shortcut.logical_key == k
                            && want.command == (modifiers.command || modifiers.ctrl)
                            && want.alt == modifiers.alt
                            && (!want.shift || modifiers.shift)
                    });
                    match found {
                        Some(b) => {
                            hit = Some(b.action);
                            false
                        }
                        None => true,
                    }
                }
                _ => true,
            });
            hit
        });
        let Some(action) = hit else {
            return false;
        };
        let (action, params) = self.shell.shortcut_action(action);
        self.shell.note_shortcut_use(action);
        events.push(Event::Action(action, params));
        true
    }

    /// The clipboard events, which the platform layer makes of Ctrl+C, Ctrl+X and Ctrl+V (egui-winit), not key
    /// presses: on the canvas the clipboard actions (a paste with the system clipboard's text, by `pasting` when the
    /// canvas's action asked for it), in a navigator's table its copy and paste, in a navigator's list a paste onto the
    /// page shown.
    /// A text field never gets here (the routing ended before), so the grid and the find pane keep egui's own copy and
    /// paste. `true` when one was taken.
    fn clipboard_keys(
        &mut self,
        ctx: &egui::Context,
        focused: Context,
        pasting: Option<&'static str>,
        events: &mut Vec<Event>,
    ) -> bool {
        if focused == Context::Canvas {
            let clip = take_clip_event(ctx, |e| {
                matches!(e, egui::Event::Copy | egui::Event::Cut | egui::Event::Paste(_))
            });
            // In the symbol editor they run its own clipboard (`shortcut_action`).
            let routed = |shell: &ShellModel, action: &'static str, params: Value| {
                let (to, own) = shell.shortcut_action(action);
                if to == action {
                    Event::Action(action, params)
                } else {
                    Event::Action(to, own)
                }
            };
            match clip {
                Some(egui::Event::Copy) => {
                    events.push(routed(&self.shell, "edit.copy", Value::Null));
                    return true;
                }
                Some(egui::Event::Cut) => {
                    events.push(routed(&self.shell, "edit.cut", Value::Null));
                    return true;
                }
                Some(egui::Event::Paste(text)) => {
                    let action = pasting
                        .filter(|a| ew_viewmodel::actions::find(a).is_some_and(|a| a.context == Context::Canvas))
                        .unwrap_or("edit.paste_clip");
                    events.push(routed(&self.shell, action, json!({ "text": text })));
                    return true;
                }
                _ => {}
            }
        }
        // In a navigator's table the clipboard events run the table's copy and paste.
        if focused == Context::Navigator
            && let Some(kind) = self.shell.focused_navigator()
            && self.shell.nav(kind).is_some_and(|n| n.table_mode().is_some())
        {
            let clip = take_clip_event(ctx, |e| matches!(e, egui::Event::Copy | egui::Event::Paste(_)));
            match clip {
                Some(egui::Event::Copy) => {
                    events.push(Event::Action("table.copy", json!({ "kind": kind.word() })));
                    return true;
                }
                Some(egui::Event::Paste(text)) => {
                    events.push(Event::Action(
                        "table.paste",
                        json!({ "kind": kind.word(), "text": text }),
                    ));
                    return true;
                }
                _ => {}
            }
        }
        // Ctrl+V with the focus in a navigator's list (no table) pastes onto the page shown (after a page was
        // opened from the page list, Ctrl+V did nothing and said nothing).
        if focused == Context::Navigator
            && let Some(egui::Event::Paste(text)) =
                take_clip_event(ctx, |e| matches!(e, egui::Event::Paste(_)))
        {
            events.push(Event::Action("edit.paste_clip", json!({ "text": text })));
            return true;
        }
        false
    }

    /// The chords matched by their logical key, the focused context's first ([`candidates`]); the palette lets
    /// only its own toggle through, and a navigator, Properties and the find pane keep their own keys.
    fn bound_keys(
        &mut self,
        ctx: &egui::Context,
        focused: Context,
        palette_open: bool,
        events: &mut Vec<Event>,
    ) {
        for binding in candidates(
            &self.bindings,
            self.shell.symbol_editor().is_some(),
            false,
            focused,
        ) {
            // Plain Escape is `ui.dismiss`'s own path above; a `ui.dismiss` the keymap binds to another key is matched
            // here, as the physical-key loop matches one.
            let escape =
                binding.shortcut.logical_key == Key::Escape && binding.shortcut.modifiers == Modifiers::NONE;
            if binding.action == "help.documentation"
                || (binding.action == "ui.dismiss" && escape)
                || (palette_open && binding.action != "tools.palette")
                || (focused == Context::Navigator && keys::is_navigator_key(&binding.shortcut))
                || (focused == Context::Properties
                    && binding.context != Context::Properties
                    && crate::properties::is_grid_key(&binding.shortcut))
                // The find pane's own keys (the arrows, Home, End, Enter) stay with it.
                || (focused == Context::Find
                    && binding.context != Context::Find
                    && crate::properties::is_grid_key(&binding.shortcut))
            {
                continue;
            }
            if ctx.input_mut(|i| i.consume_shortcut(&binding.shortcut)) {
                // A chord that sets a parameter sends its place (`rail.entry`'s Ctrl+1 to Ctrl+9 pick the left rail's
                // entries in their order, doc 16a §5.4).
                if let Some((param, n)) = binding.sets {
                    let mut params = serde_json::Map::new();
                    params.insert(param.to_owned(), n.into());
                    events.push(Event::Action(binding.action, Value::Object(params)));
                    return;
                }
                let (action, params) = self.shell.shortcut_action(binding.action);
                // A shortcut invocation counts for the palette's recents.
                self.shell.note_shortcut_use(action);
                events.push(Event::Action(action, params));
                return;
            }
        }
    }

    /// Alt pressed and released with nothing else, or F10, opens the app menu (doc 16a §5.2); not while a dialog or a
    /// file dialog is open.
    pub(super) fn app_menu_keys(&mut self, ctx: &egui::Context) {
        let (alt, other, f10) = ctx.input_mut(|i| {
            let other = i.events.iter().any(|e| {
                matches!(
                    e,
                    egui::Event::Key { pressed: true, .. }
                        | egui::Event::PointerButton { pressed: true, .. }
                        | egui::Event::Text(_)
                )
            });
            // F10 alone: `consume_key` matches logically and would take Shift+F10, the context menu key (FR-UI-031).
            let f10 = i.modifiers.is_none() && i.consume_key(Modifiers::NONE, Key::F10);
            (i.modifiers.alt, other, f10)
        });
        let mut open = f10;
        if alt && !self.alt_down {
            self.alt_alone = !other;
        } else if alt && other {
            self.alt_alone = false;
        }
        if !alt && self.alt_down && self.alt_alone {
            open = true;
        }
        if !alt {
            self.alt_alone = false;
        }
        self.alt_down = alt;
        if open && self.shell.dialog().is_none() && self.pending_pick.is_none() {
            egui::Popup::open_id(ctx, crate::command_bar::app_menu_id());
        }
    }
}

/// The bindings of `bindings` matched by their physical key (`physical`) or by their logical one, in the global context
/// and in `focused`, most modifiers first (egui matches an extra Shift or Alt logically, so Alt+Left must be asked
/// before Left), the focused context first on a tie. While the symbol editor shows (`editor`), of the canvas's keys
/// those that choose its tools apply, and those that act on its selection as they do on a page's (the arrows, Delete,
/// the clipboard, Select all).
fn candidates(bindings: &[Binding], editor: bool, physical: bool, focused: Context) -> Vec<&Binding> {
    use ew_viewmodel::shell::symbol_editor::{EDIT_KEYS, TOOL_KEYS};
    let applies = |b: &Binding| {
        !editor
            || b.context != Context::Canvas
            || TOOL_KEYS.iter().any(|(_, action)| *action == b.action)
            || EDIT_KEYS.iter().any(|(action, ..)| *action == b.action)
    };
    let mut candidates: Vec<&Binding> = bindings
        .iter()
        .filter(|b| {
            b.physical == physical && (b.context == Context::Global || b.context == focused) && applies(b)
        })
        .collect();
    candidates.sort_by_key(|b| {
        let m = b.shortcut.modifiers;
        (
            std::cmp::Reverse(usize::from(m.shift) + usize::from(m.alt) + usize::from(m.command)),
            b.context != focused,
        )
    });
    candidates
}

/// Takes the first of the frame's input events that `wanted` picks (a clipboard event), the others left in place.
fn take_clip_event(ctx: &egui::Context, wanted: impl Fn(&egui::Event) -> bool) -> Option<egui::Event> {
    ctx.input_mut(|i| {
        let mut hit = None;
        i.events.retain(|e| {
            if hit.is_none() && wanted(e) {
                hit = Some(e.clone());
                false
            } else {
                true
            }
        });
        hit
    })
}
