// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Keyboard shortcuts of the registry as egui shortcuts (WP-S2.01 part 1, spec section 5.3, UX/UI doc 16
//! section 4.2).
//!
//! The registry writes bindings as text (`Ctrl+Shift+Z`, `Page Down`); on macOS `Ctrl` reads `Cmd`, which egui
//! calls `COMMAND`. Routing order: a focused text field, then an open popup or dialog, then the shell's
//! shortcuts; Escape closes the topmost popup or dialog and otherwise does nothing.

use egui::{Key, KeyboardShortcut, Modifiers};
use ew_viewmodel::actions::{ACTIONS, Context};

/// A binding and the action it runs.
#[derive(Clone, Copy, Debug)]
pub struct Binding {
    /// The shortcut.
    pub shortcut: KeyboardShortcut,
    /// The action ID.
    pub action: &'static str,
    /// Where the chord applies (WP-S2.03 §6.6).
    pub context: Context,
}

/// Whether `s` is one of the page navigator's own keys (the arrows, Home, End, Enter and Ctrl+Enter), which a global
/// chord must not take while the navigator has the focus.
pub fn is_navigator_key(s: &KeyboardShortcut) -> bool {
    let plain = s.modifiers == Modifiers::NONE;
    let nav = matches!(
        s.logical_key,
        Key::ArrowUp | Key::ArrowDown | Key::ArrowLeft | Key::ArrowRight | Key::Home | Key::End | Key::Enter
    );
    nav && (plain || (s.modifiers == Modifiers::COMMAND && s.logical_key == Key::Enter))
}

/// The shortcut of a binding text such as `Ctrl+Shift+Z` or `Page Down`; `None` for a text with no known key.
pub fn parse_shortcut(text: &str) -> Option<KeyboardShortcut> {
    let mut modifiers = Modifiers::NONE;
    let mut key = None;
    for part in text.split('+') {
        match part.trim() {
            "Ctrl" => modifiers |= Modifiers::COMMAND,
            "Shift" => modifiers |= Modifiers::SHIFT,
            "Alt" => modifiers |= Modifiers::ALT,
            name => key = Key::from_name(&name.replace(' ', "")),
        }
    }
    key.map(|k| KeyboardShortcut::new(modifiers, k))
}

/// The registry's bindings, most specific first (egui matches extra Shift and Alt logically, so `Ctrl+Shift+S` must
/// be asked before `Ctrl+S`).
pub fn bindings() -> Vec<Binding> {
    let mut out: Vec<Binding> = ACTIONS
        .iter()
        .flat_map(|meta| {
            meta.shortcut.iter().filter_map(move |text| {
                parse_shortcut(text).map(|shortcut| Binding {
                    shortcut,
                    action: meta.id,
                    context: meta.context,
                })
            })
        })
        .collect();
    out.sort_by_key(|b| {
        let m = b.shortcut.modifiers;
        std::cmp::Reverse(usize::from(m.shift) + usize::from(m.alt) + usize::from(m.command))
    });
    out
}

/// The text of a binding for display: `Ctrl` reads `Cmd` on macOS.
pub fn display(text: &str) -> String {
    if cfg!(target_os = "macos") {
        text.replace("Ctrl", "Cmd")
    } else {
        text.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_binding_of_the_registry_parses() {
        let expected: usize = ACTIONS.iter().map(|a| a.shortcut.len()).sum();
        assert_eq!(bindings().len(), expected);
    }

    #[test]
    fn specific_bindings_come_first() {
        let all = bindings();
        let at = |action: &str, shift: bool| {
            all.iter()
                .position(|b| b.action == action && b.shortcut.modifiers.shift == shift)
                .unwrap()
        };
        assert!(at("file.save_as", true) < at("file.save", false));
        assert!(at("edit.redo", true) < at("edit.undo", false));
    }

    #[test]
    fn page_keys_and_letters_parse() {
        let s = parse_shortcut("Page Down").unwrap();
        assert_eq!(s.logical_key, Key::PageDown);
        assert_eq!(s.modifiers, Modifiers::NONE);
        let s = parse_shortcut("Ctrl+Shift+Z").unwrap();
        assert_eq!(s.logical_key, Key::Z);
        assert!(s.modifiers.shift && s.modifiers.command);
        assert!(parse_shortcut("Ctrl+NoSuchKey").is_none());
    }
}
