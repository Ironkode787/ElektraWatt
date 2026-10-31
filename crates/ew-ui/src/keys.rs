// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Keyboard shortcuts of the keymap as egui shortcuts (WP-S2.01 part 1, spec section 5.3, UX/UI doc 16
//! section 4.2; WP-S2.08 part 2, D6).
//!
//! The keymap writes bindings as text (`Ctrl+Shift+Z`, `Page Down`, `]`, `phys:BracketRight`); on macOS `Ctrl` reads
//! `Cmd`, which egui calls `COMMAND`. Letters, digits and named keys match by logical key; a punctuation character and
//! a `phys:` key match by physical key ([`Binding::physical`]), so `]` needs no AltGr on QWERTZ and AZERTY. Routing
//! order: a focused text field, then an open popup or dialog, then the shell's shortcuts; Escape closes the topmost
//! popup or dialog and otherwise does nothing.

use egui::{Key, KeyboardShortcut, Modifiers};
use ew_viewmodel::actions::Context;
use ew_viewmodel::keymap::{Bound, Chord, ChordKey};

/// A binding and the action it runs.
#[derive(Clone, Copy, Debug)]
pub struct Binding {
    /// The shortcut; for a physical binding its key is the physical key.
    pub shortcut: KeyboardShortcut,
    /// Whether the key matches by its position (egui's `physical_key`) rather than by what it types.
    pub physical: bool,
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

/// The egui key of a physical key code (`BracketRight`, `Digit0`, `KeyA`); `None` for one egui does not name.
pub fn physical_key(code: &str) -> Option<Key> {
    Some(match code {
        "Backquote" => Key::Backtick,
        "Minus" => Key::Minus,
        "Equal" => Key::Equals,
        "BracketLeft" => Key::OpenBracket,
        "BracketRight" => Key::CloseBracket,
        "Backslash" => Key::Backslash,
        "Semicolon" => Key::Semicolon,
        "Quote" => Key::Quote,
        "Comma" => Key::Comma,
        "Period" => Key::Period,
        "Slash" => Key::Slash,
        other => {
            let name = other
                .strip_prefix("Digit")
                .or_else(|| other.strip_prefix("Key"))?;
            return Key::from_name(name);
        }
    })
}

/// The physical key code of an egui key that sits on one key of every layout (the inverse of [`physical_key`] for the
/// punctuation keys).
fn punctuation_code(key: Key) -> Option<&'static str> {
    Some(match key {
        Key::Backtick => "Backquote",
        Key::Minus => "Minus",
        Key::Equals => "Equal",
        Key::OpenBracket => "BracketLeft",
        Key::CloseBracket => "BracketRight",
        Key::Backslash => "Backslash",
        Key::Semicolon => "Semicolon",
        Key::Quote => "Quote",
        Key::Comma => "Comma",
        Key::Period => "Period",
        Key::Slash => "Slash",
        _ => return None,
    })
}

/// The egui shortcut of `chord` and whether it is physical; `None` for a key egui does not name.
pub fn shortcut_of(chord: &Chord) -> Option<(KeyboardShortcut, bool)> {
    let mut modifiers = Modifiers::NONE;
    if chord.ctrl {
        modifiers |= Modifiers::COMMAND;
    }
    if chord.shift {
        modifiers |= Modifiers::SHIFT;
    }
    if chord.alt {
        modifiers |= Modifiers::ALT;
    }
    // The Windows key is reserved (D8): egui does not report it as a modifier, so such a chord never binds.
    if chord.win {
        return None;
    }
    match &chord.key {
        ChordKey::Logical(name) => {
            Key::from_name(&name.replace(' ', "")).map(|k| (KeyboardShortcut::new(modifiers, k), false))
        }
        ChordKey::Physical(code) => physical_key(code).map(|k| (KeyboardShortcut::new(modifiers, k), true)),
    }
}

/// The shortcut of a binding text such as `Ctrl+Shift+Z` or `Page Down`; `None` for a text with no known key.
pub fn parse_shortcut(text: &str) -> Option<KeyboardShortcut> {
    Chord::parse(text)
        .ok()
        .and_then(|c| shortcut_of(&c))
        .map(|(s, _)| s)
}

/// The keymap's effective bindings `bound` as egui bindings, most specific first (egui matches extra Shift and Alt
/// logically, so `Ctrl+Shift+S` must be asked before `Ctrl+S`).
pub fn bindings(bound: &[Bound]) -> Vec<Binding> {
    let mut out: Vec<Binding> = bound
        .iter()
        .filter_map(|b| {
            shortcut_of(&b.chord).map(|(shortcut, physical)| Binding {
                shortcut,
                physical,
                action: b.action,
                context: b.context,
            })
        })
        .collect();
    out.sort_by_key(|b| {
        let m = b.shortcut.modifiers;
        std::cmp::Reverse(usize::from(m.shift) + usize::from(m.alt) + usize::from(m.command))
    });
    out
}

/// The binding text of a key press for the keymap editor's recording: `None` for a modifier alone or a key the
/// keymap cannot name. A punctuation key records its physical key (D6), every other key its logical name.
pub fn chord_text(key: Key, physical: Option<Key>, modifiers: Modifiers) -> Option<String> {
    let name = match punctuation_code(physical.unwrap_or(key)).or_else(|| punctuation_code(key)) {
        Some(code) => format!("phys:{code}"),
        None => {
            let name = key.name();
            match name {
                "Up" | "Down" | "Left" | "Right" => format!("Arrow {name}"),
                "PageUp" => "Page Up".to_owned(),
                "PageDown" => "Page Down".to_owned(),
                _ => name.to_owned(),
            }
        }
    };
    let mut text = String::new();
    for (on, m) in [
        (modifiers.command || modifiers.ctrl, "Ctrl+"),
        (modifiers.alt, "Alt+"),
        (modifiers.shift, "Shift+"),
    ] {
        if on {
            text.push_str(m);
        }
    }
    text.push_str(&name);
    // Only what the keymap can name (and print back) is recorded.
    Chord::parse(&text).ok().map(|c| c.text())
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
    use ew_viewmodel::actions::ACTIONS;
    use ew_viewmodel::keymap::{Preset, effective};

    #[test]
    fn every_binding_of_both_presets_parses() {
        for preset in Preset::ALL {
            let bound = effective(preset, None);
            assert_eq!(bindings(&bound).len(), bound.len(), "{preset:?}");
        }
        let expected: usize = ACTIONS.iter().map(|a| a.shortcut.len()).sum();
        assert_eq!(effective(Preset::Ew, None).len(), expected);
    }

    #[test]
    fn specific_bindings_come_first() {
        let all = bindings(&effective(Preset::Ew, None));
        let at = |action: &str, shift: bool| {
            all.iter()
                .position(|b| b.action == action && b.shortcut.modifiers.shift == shift)
                .unwrap()
        };
        assert!(at("file.save_as", true) < at("file.save", false));
        assert!(at("edit.redo", true) < at("edit.undo", false));
    }

    #[test]
    fn page_keys_letters_and_physical_keys_parse() {
        let s = parse_shortcut("Page Down").unwrap();
        assert_eq!(s.logical_key, Key::PageDown);
        assert_eq!(s.modifiers, Modifiers::NONE);
        let s = parse_shortcut("Ctrl+Shift+Z").unwrap();
        assert_eq!(s.logical_key, Key::Z);
        assert!(s.modifiers.shift && s.modifiers.command);
        assert!(parse_shortcut("Ctrl+NoSuchKey").is_none());
        // `]` and `phys:BracketRight` are one physical binding (D6).
        let bracket = Chord::parse("]").unwrap();
        assert_eq!(
            shortcut_of(&bracket).map(|(s, p)| (s.logical_key, p)),
            Some((Key::CloseBracket, true))
        );
        let digit = Chord::parse("Ctrl+phys:Digit0").unwrap();
        assert_eq!(
            shortcut_of(&digit).map(|(s, p)| (s.logical_key, p)),
            Some((Key::Num0, true))
        );
        assert!(shortcut_of(&Chord::parse("Win+E").unwrap()).is_none());
    }

    #[test]
    fn a_key_press_records_as_the_keymap_writes_it() {
        assert_eq!(
            chord_text(Key::W, Some(Key::W), Modifiers::COMMAND | Modifiers::SHIFT),
            Some("Ctrl+Shift+W".to_owned())
        );
        // On QWERTZ the key right of P types `+`; it records as its physical key, `]`.
        assert_eq!(
            chord_text(Key::Plus, Some(Key::CloseBracket), Modifiers::NONE),
            Some("]".to_owned())
        );
        assert_eq!(
            chord_text(Key::PageDown, None, Modifiers::NONE),
            Some("Page Down".to_owned())
        );
        assert_eq!(
            chord_text(Key::ArrowLeft, None, Modifiers::ALT),
            Some("Alt+Arrow Left".to_owned())
        );
        assert_eq!(
            chord_text(Key::F8, None, Modifiers::SHIFT),
            Some("Shift+F8".to_owned())
        );
    }
}
