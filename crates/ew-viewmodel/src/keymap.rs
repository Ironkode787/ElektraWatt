// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Keymaps (WP-S2.08 part 2, D6 to D8; FR-UI-013, NFR-I18N-06; doc 16 §4.4 and §15.5).
//!
//! **Chords.** A binding text is modifiers in a fixed order (`Ctrl`, `Alt`, `Shift`, `Win`) and a key. Letters and
//! digits, the named keys (`Enter`, `Page Down`, `F8`) and the registry's `Plus`, `Minus` and `Equals` bind by logical
//! key; a punctuation character (`]`) and `phys:<code>` (`phys:BracketRight`, W3C `KeyboardEvent.code` names) bind by
//! physical key, so `]` is the key right of P whatever the layout prints on it. The registry and the window show the
//! character form (`]`); the keymap files write the `phys:` form (D6). [`Chord`] is the normalised form that the
//! conflict rules compare.
//!
//! **Keymaps.** A keymap file is doc 16 §4.4's TOML: `[meta]` with `name` and `version = 1`, then `[[bind]]` tables
//! with `keys`, `action`, `context` (one of the five shell contexts in lower case) and an optional `alt`. The EW preset
//! is generated from the registry's `ActionMeta.shortcut` (the registry stays its source; `keymaps/ew.toml` is its
//! written form, kept equal by a test); the KiCad-like preset is the EW preset with `A` for Place (replacing `I`) and
//! `G` beside `M` for Move. The user's `keymap.toml` in the settings directory replaces the preset's bindings of the
//! actions it names; a binding for an action this build does not know is kept and listed as unused.
//!
//! **Rules** (D8, doc 16 §15.5 over the contexts that exist, `global` the parent of the four others): equal chords in
//! one context are a conflict (an error); a child context's chord over a global one is shadowing (an info);
//! Ctrl+Alt+any key and the reserved chords (Alt+F4, Alt+Tab, Win+anything) are errors. **Reachability:** for
//! QWERTY-US, QWERTZ-DE and AZERTY-FR the characters on the base and Shift levels of each physical key; a binding is
//! typable on a layout when its key is on one of those levels (never AltGr), or it binds by physical key or by a
//! layout-independent name; a binding whose character needs Shift on a layout while the chord has no Shift is an
//! error there; every action bound in a preset needs one typable binding per layout.

use std::path::Path;

use crate::actions::{ACTIONS, ActionMeta, Context};
use crate::strings;

/// The user's keymap file inside the settings directory (D6).
pub const USER_FILE: &str = "keymap.toml";

/// The version a keymap file carries in `[meta]`.
pub const FORMAT_VERSION: i64 = 1;

/// The named keys that bind by logical key, as the registry writes them. Letters and digits are single characters.
const NAMED: &[&str] = &[
    "Enter",
    "Escape",
    "Tab",
    "Space",
    "Backspace",
    "Delete",
    "Insert",
    "Home",
    "End",
    "Page Up",
    "Page Down",
    "Arrow Up",
    "Arrow Down",
    "Arrow Left",
    "Arrow Right",
    "Plus",
    "Minus",
    "Equals",
    "F1",
    "F2",
    "F3",
    "F4",
    "F5",
    "F6",
    "F7",
    "F8",
    "F9",
    "F10",
    "F11",
    "F12",
];

/// The physical keys of the punctuation and the character the US layout prints on each: `]` in a binding text names
/// `BracketRight`.
const PUNCTUATION: &[(&str, char)] = &[
    ("Backquote", '`'),
    ("Minus", '-'),
    ("Equal", '='),
    ("BracketLeft", '['),
    ("BracketRight", ']'),
    ("Backslash", '\\'),
    ("Semicolon", ';'),
    ("Quote", '\''),
    ("Comma", ','),
    ("Period", '.'),
    ("Slash", '/'),
];

/// Whether `code` is a physical key a binding may name: the punctuation keys, `Digit0` to `Digit9` and `KeyA` to
/// `KeyZ`.
fn physical_code_known(code: &str) -> bool {
    PUNCTUATION.iter().any(|(c, _)| *c == code)
        || code
            .strip_prefix("Digit")
            .is_some_and(|d| d.len() == 1 && d.as_bytes()[0].is_ascii_digit())
        || code
            .strip_prefix("Key")
            .is_some_and(|d| d.len() == 1 && d.as_bytes()[0].is_ascii_uppercase())
}

/// The key of a chord.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum ChordKey {
    /// By the character or name the layout produces: `W`, `0`, `F8`, `Page Down`, `Plus`.
    Logical(String),
    /// By the physical key, a W3C code name: `BracketRight`, `Digit0`.
    Physical(String),
}

/// A normalised key chord (doc 16 §15.5): two chords are the same binding exactly when they are equal.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct Chord {
    /// Ctrl (Cmd on macOS).
    pub ctrl: bool,
    /// Alt.
    pub alt: bool,
    /// Shift.
    pub shift: bool,
    /// The Windows (Super) key.
    pub win: bool,
    /// The key.
    pub key: ChordKey,
}

impl Chord {
    /// The chord of a binding text, in the registry's form (`Ctrl+Shift+Z`, `]`) or the file's (`phys:BracketRight`);
    /// a letter may be lower case. `Err` carries the English reason.
    ///
    /// # Errors
    ///
    /// An empty part, an unknown modifier or key, or a modifier written twice.
    pub fn parse(text: &str) -> Result<Chord, String> {
        let parts: Vec<&str> = text.split('+').map(str::trim).collect();
        let Some((key, modifiers)) = parts.split_last() else {
            return Err(strings::format("keymap-error-key", &[text]));
        };
        let mut chord = Chord {
            ctrl: false,
            alt: false,
            shift: false,
            win: false,
            key: ChordKey::Logical(String::new()),
        };
        for m in modifiers {
            let flag = match m.to_ascii_lowercase().as_str() {
                "ctrl" => &mut chord.ctrl,
                "alt" => &mut chord.alt,
                "shift" => &mut chord.shift,
                "win" | "super" | "meta" => &mut chord.win,
                _ => return Err(strings::format("keymap-error-modifier", &[m, text])),
            };
            if *flag {
                return Err(strings::format("keymap-error-modifier", &[m, text]));
            }
            *flag = true;
        }
        chord.key = parse_key(key).ok_or_else(|| strings::format("keymap-error-key", &[text]))?;
        Ok(chord)
    }

    /// The modifiers as text, each followed by `+`.
    fn modifier_text(&self) -> String {
        let mut out = String::new();
        for (on, name) in [
            (self.ctrl, "Ctrl+"),
            (self.alt, "Alt+"),
            (self.shift, "Shift+"),
            (self.win, "Win+"),
        ] {
            if on {
                out.push_str(name);
            }
        }
        out
    }

    /// The text the registry, the menus and the window show: a punctuation key as its US character (`]`), another
    /// physical key in the `phys:` form.
    pub fn text(&self) -> String {
        let key = match &self.key {
            ChordKey::Logical(name) => name.clone(),
            ChordKey::Physical(code) => PUNCTUATION
                .iter()
                .find(|(c, _)| c == code)
                .map_or_else(|| format!("phys:{code}"), |(_, ch)| ch.to_string()),
        };
        format!("{}{key}", self.modifier_text())
    }

    /// The text a keymap file writes: a physical key always in the `phys:` form (D6).
    pub fn file_text(&self) -> String {
        let key = match &self.key {
            ChordKey::Logical(name) => name.clone(),
            ChordKey::Physical(code) => format!("phys:{code}"),
        };
        format!("{}{key}", self.modifier_text())
    }
}

/// The key of a binding text's last part; `None` for an unknown key.
fn parse_key(key: &str) -> Option<ChordKey> {
    if let Some(code) = key.strip_prefix("phys:") {
        return physical_code_known(code).then(|| ChordKey::Physical(code.to_owned()));
    }
    let mut chars = key.chars();
    if let (Some(c), None) = (chars.next(), chars.clone().next()) {
        if c.is_ascii_alphabetic() {
            return Some(ChordKey::Logical(c.to_ascii_uppercase().to_string()));
        }
        if c.is_ascii_digit() {
            return Some(ChordKey::Logical(c.to_string()));
        }
        return PUNCTUATION
            .iter()
            .find(|(_, ch)| *ch == c)
            .map(|(code, _)| ChordKey::Physical((*code).to_owned()));
    }
    NAMED
        .iter()
        .find(|n| n.eq_ignore_ascii_case(key))
        .map(|n| ChordKey::Logical((*n).to_owned()))
}

/// The word of a context in a keymap file.
pub fn context_word(context: Context) -> &'static str {
    match context {
        Context::Global => "global",
        Context::Canvas => "canvas",
        Context::Navigator => "navigator",
        Context::Properties => "properties",
        Context::Find => "find",
    }
}

/// The context of a keymap file's word.
pub fn context_of_word(word: &str) -> Option<Context> {
    [
        Context::Global,
        Context::Canvas,
        Context::Navigator,
        Context::Properties,
        Context::Find,
    ]
    .into_iter()
    .find(|c| context_word(*c) == word)
}

// ---------------------------------------------------------------------------------------------------------------
// Presets

/// A keymap preset (D6); the settings name it with [`Preset::word`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Preset {
    /// EW: the registry's bindings.
    Ew,
    /// KiCad-like: the EW preset with `A` for Place (replacing `I`) and `G` beside `M` for Move.
    Kicad,
}

impl Preset {
    /// The presets, in the settings' order.
    pub const ALL: [Preset; 2] = [Preset::Ew, Preset::Kicad];

    /// The preset of the settings value `keymap`; anything but `kicad` is the EW preset.
    pub fn from_setting(word: &str) -> Preset {
        if word == "kicad" {
            Preset::Kicad
        } else {
            Preset::Ew
        }
    }

    /// The settings value (`ew`, `kicad`), also the file name's stem in `keymaps/`.
    pub fn word(self) -> &'static str {
        match self {
            Preset::Ew => "ew",
            Preset::Kicad => "kicad",
        }
    }

    /// The name its file's `[meta]` carries and the window shows.
    pub fn name(self) -> &'static str {
        match self {
            Preset::Ew => "EW",
            Preset::Kicad => "KiCad-like",
        }
    }

    /// The preset's bindings of `meta`, as binding texts.
    pub fn shortcuts(self, meta: &ActionMeta) -> &'static [&'static str] {
        match (self, meta.id) {
            (Preset::Kicad, "tool.place") => &["A"],
            (Preset::Kicad, "tool.move") => &["M", "G"],
            _ => meta.shortcut,
        }
    }

    /// The preset as a keymap file: one `[[bind]]` per action with bindings, in registry order, its first binding as
    /// `keys` and its second as `alt` (a third and later as further `[[bind]]` tables).
    pub fn keymap(self) -> Keymap {
        let mut binds = Vec::new();
        for meta in ACTIONS {
            let chords: Vec<Chord> = self
                .shortcuts(meta)
                .iter()
                .filter_map(|s| Chord::parse(s).ok())
                .collect();
            push_binds(&mut binds, meta.id, meta.context, &chords);
        }
        Keymap {
            name: self.name().to_owned(),
            binds,
        }
    }
}

/// Appends `chords` of `action` in `context` as `[[bind]]` tables of two chords each (`keys`, then `alt`).
fn push_binds(binds: &mut Vec<Bind>, action: &str, context: Context, chords: &[Chord]) {
    for pair in chords.chunks(2) {
        binds.push(Bind {
            keys: pair[0].clone(),
            action: action.to_owned(),
            context,
            alt: pair.get(1).cloned(),
        });
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Keymap files

/// One `[[bind]]` of a keymap file.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Bind {
    /// The chord.
    pub keys: Chord,
    /// The action ID (kept when this build does not know it).
    pub action: String,
    /// Where the chord applies.
    pub context: Context,
    /// A second chord for keyboards without the first key.
    pub alt: Option<Chord>,
}

impl Bind {
    /// The bind's chords: `keys`, then `alt`.
    pub fn chords(&self) -> impl Iterator<Item = &Chord> {
        std::iter::once(&self.keys).chain(&self.alt)
    }
}

/// A keymap file (doc 16 §4.4).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Keymap {
    /// `[meta] name`.
    pub name: String,
    /// The `[[bind]]` tables, in file order.
    pub binds: Vec<Bind>,
}

impl Keymap {
    /// The keymap of a TOML text. `Err` carries the English reason, naming the `[[bind]]` by its position.
    ///
    /// # Errors
    ///
    /// Text that is not TOML, a missing or wrong `[meta]` (`version` must be [`FORMAT_VERSION`]), or a `[[bind]]`
    /// without `keys`, `action` or `context`, with an unknown key or context, or with a member of another type.
    pub fn parse(text: &str) -> Result<Keymap, String> {
        let table: toml::Table =
            toml::from_str(text).map_err(|_| strings::text("keymap-error-toml").to_owned())?;
        let meta = match table.get("meta") {
            Some(toml::Value::Table(m)) => m,
            _ => return Err(strings::text("keymap-error-meta").to_owned()),
        };
        if meta.get("version").and_then(toml::Value::as_integer) != Some(FORMAT_VERSION) {
            return Err(strings::text("keymap-error-meta").to_owned());
        }
        let name = meta
            .get("name")
            .and_then(toml::Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let items = match table.get("bind") {
            None => &Vec::new(),
            Some(toml::Value::Array(items)) => items,
            Some(_) => return Err(strings::format("keymap-error-bind", &["1"])),
        };
        let mut binds = Vec::new();
        for (i, item) in items.iter().enumerate() {
            let n = (i + 1).to_string();
            let bad = || strings::format("keymap-error-bind", &[&n]);
            let toml::Value::Table(b) = item else {
                return Err(bad());
            };
            let text_of = |key: &str| b.get(key).and_then(toml::Value::as_str);
            let (Some(keys), Some(action), Some(context)) =
                (text_of("keys"), text_of("action"), text_of("context"))
            else {
                return Err(bad());
            };
            if b.get("alt").is_some_and(|v| v.as_str().is_none()) {
                return Err(bad());
            }
            let context = context_of_word(context)
                .ok_or_else(|| strings::format("keymap-error-context", &[&n, context]))?;
            let keys = Chord::parse(keys).map_err(|e| format!("{} {e}", bad()))?;
            let alt = match text_of("alt") {
                Some(a) => Some(Chord::parse(a).map_err(|e| format!("{} {e}", bad()))?),
                None => None,
            };
            binds.push(Bind {
                keys,
                action: action.to_owned(),
                context,
                alt,
            });
        }
        Ok(Keymap { name, binds })
    }

    /// The file text: `[meta]`, then one `[[bind]]` per bind in order, `keys`, `action`, `context` and `alt` in that
    /// order. Deterministic; [`Keymap::parse`] gives the keymap back.
    pub fn to_toml(&self) -> String {
        let quote = |s: &str| toml::Value::String(s.to_owned()).to_string();
        let mut out = format!(
            "[meta]\nname = {}\nversion = {FORMAT_VERSION}\n",
            quote(&self.name)
        );
        for b in &self.binds {
            out.push_str(&format!(
                "\n[[bind]]\nkeys = {}\naction = {}\ncontext = {}\n",
                quote(&b.keys.file_text()),
                quote(&b.action),
                quote(context_word(b.context))
            ));
            if let Some(alt) = &b.alt {
                out.push_str(&format!("alt = {}\n", quote(&alt.file_text())));
            }
        }
        out
    }

    /// The binds whose action this build does not know (D6: kept and listed as unused).
    pub fn unused(&self) -> Vec<&Bind> {
        self.binds
            .iter()
            .filter(|b| crate::actions::find(&b.action).is_none())
            .collect()
    }
}

/// Why the user's keymap file could not be used ([`read_user`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum UserFileError {
    /// The file is there but could not be read (locked by an antivirus or another instance, no permission): it may
    /// be valid, so it is neither copied aside nor written over. The I/O error in words.
    Read(String),
    /// The file was read but is not a keymap: the reason of [`Keymap::parse`], or that it is not UTF-8.
    Invalid(String),
}

/// Reads the user's keymap `<dir>/keymap.toml`: `Ok(None)` only when there is no file (`NotFound`).
///
/// # Errors
///
/// [`UserFileError::Read`] for any other read error, [`UserFileError::Invalid`] when the file does not parse.
pub fn read_user(dir: &Path) -> Result<Option<Keymap>, UserFileError> {
    let bytes = match std::fs::read(dir.join(USER_FILE)) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(UserFileError::Read(e.to_string())),
    };
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| UserFileError::Invalid(strings::text("keymap-error-toml").to_owned()))?;
    Keymap::parse(text).map(Some).map_err(UserFileError::Invalid)
}

/// Writes the user's keymap as `<dir>/keymap.toml` through a temporary file and a rename; creates `dir` if needed.
///
/// # Errors
///
/// The I/O error of the first step that failed.
pub fn write_user(dir: &Path, keymap: &Keymap) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!("{USER_FILE}.tmp"));
    std::fs::write(&tmp, keymap.to_toml())?;
    std::fs::rename(&tmp, dir.join(USER_FILE))
}

// ---------------------------------------------------------------------------------------------------------------
// The effective bindings

/// One effective binding: a chord of a known action in a context.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Bound {
    /// The action.
    pub action: &'static str,
    /// The chord.
    pub chord: Chord,
    /// Where it applies.
    pub context: Context,
}

/// The chords of `meta` under `preset` and the user's keymap `user`: the user's binds of the action when it names it
/// (D6: they replace the preset's), else the preset's, each with its context.
pub fn chords_of(preset: Preset, user: Option<&Keymap>, meta: &ActionMeta) -> Vec<(Chord, Context)> {
    if let Some(user) = user
        && user.binds.iter().any(|b| b.action == meta.id)
    {
        return user
            .binds
            .iter()
            .filter(|b| b.action == meta.id)
            .flat_map(|b| b.chords().map(|c| (c.clone(), b.context)))
            .collect();
    }
    preset
        .shortcuts(meta)
        .iter()
        .filter_map(|s| Chord::parse(s).ok())
        .map(|c| (c, meta.context))
        .collect()
}

/// Whether the effective bindings of `meta` under `user` differ from the preset's (W40a 5b): an action whose user
/// binds equal the preset's is not changed.
pub fn is_changed(preset: Preset, user: Option<&Keymap>, meta: &ActionMeta) -> bool {
    user.is_some() && chords_of(preset, user, meta) != chords_of(preset, None, meta)
}

/// Whether any bindable action's bindings differ from the preset's under `user` (the editor's "with your changes").
pub fn has_changes(preset: Preset, user: Option<&Keymap>) -> bool {
    user.is_some()
        && ACTIONS
            .iter()
            .filter(|m| !m.hidden && !m.headless_only)
            .any(|m| is_changed(preset, user, m))
}

/// The words of a context in a sentence, as the editor's column names it ("in the canvas", "everywhere").
fn context_phrase(context: Context) -> &'static str {
    strings::text(match context {
        Context::Global => "keymap-in-global",
        Context::Canvas => "keymap-in-canvas",
        Context::Navigator => "keymap-in-navigator",
        Context::Properties => "keymap-in-properties",
        Context::Find => "keymap-in-find",
    })
}

/// The text of `finding` for a refused recording of `recorded` (W40a 5c): a conflict says what is in the way ("M is
/// already Move in the canvas."), not the double binding the refusal prevented; any other finding as
/// [`Finding::text`].
pub fn refusal_text(finding: &Finding, recorded: &str) -> String {
    match finding {
        Finding::Conflict {
            chord,
            context,
            actions,
        } if actions.contains(&recorded) => {
            let label = |id: &str| {
                crate::actions::find(id).map_or_else(|| id.to_owned(), |m| strings::text(m.label_key).to_owned())
            };
            let others: Vec<String> = actions.iter().filter(|a| **a != recorded).map(|a| label(a)).collect();
            strings::format(
                "keymap-taken",
                &[&chord.text(), &others.join(", "), context_phrase(*context)],
            )
        }
        other => other.text(),
    }
}

/// Every effective binding, in registry order (D6).
pub fn effective(preset: Preset, user: Option<&Keymap>) -> Vec<Bound> {
    ACTIONS
        .iter()
        .flat_map(|meta| {
            chords_of(preset, user, meta)
                .into_iter()
                .map(|(chord, context)| Bound {
                    action: meta.id,
                    chord,
                    context,
                })
        })
        .collect()
}

/// The effective bindings as a self-contained keymap file named `name` (the editor's export): every bound action,
/// then the user's unused binds, so that importing it gives the same bindings back.
pub fn export(name: &str, preset: Preset, user: Option<&Keymap>) -> Keymap {
    let mut binds = Vec::new();
    for meta in ACTIONS {
        let chords = chords_of(preset, user, meta);
        // Consecutive chords of one context share their `[[bind]]` tables.
        let mut start = 0;
        while start < chords.len() {
            let context = chords[start].1;
            let end = chords[start..]
                .iter()
                .position(|(_, c)| *c != context)
                .map_or(chords.len(), |n| start + n);
            let group: Vec<Chord> = chords[start..end].iter().map(|(c, _)| c.clone()).collect();
            push_binds(&mut binds, meta.id, context, &group);
            start = end;
        }
    }
    if let Some(user) = user {
        binds.extend(user.unused().into_iter().cloned());
    }
    Keymap {
        name: name.to_owned(),
        binds,
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Conflicts and reachability (D8)

/// A keyboard layout of the reachability check (NFR-I18N-06).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Layout {
    /// QWERTY, US.
    QwertyUs,
    /// QWERTZ, German.
    QwertzDe,
    /// AZERTY, French.
    AzertyFr,
}

/// The base and Shift characters of each physical key of a layout's alphanumeric block (`'\0'` for none). Letters are
/// listed on their base level in lower case.
type LayoutTable = &'static [(&'static str, char, char)];

const QWERTY_US: LayoutTable = &[
    ("Backquote", '`', '~'),
    ("Digit1", '1', '!'),
    ("Digit2", '2', '@'),
    ("Digit3", '3', '#'),
    ("Digit4", '4', '$'),
    ("Digit5", '5', '%'),
    ("Digit6", '6', '^'),
    ("Digit7", '7', '&'),
    ("Digit8", '8', '*'),
    ("Digit9", '9', '('),
    ("Digit0", '0', ')'),
    ("Minus", '-', '_'),
    ("Equal", '=', '+'),
    ("KeyQ", 'q', 'Q'),
    ("KeyW", 'w', 'W'),
    ("KeyE", 'e', 'E'),
    ("KeyR", 'r', 'R'),
    ("KeyT", 't', 'T'),
    ("KeyY", 'y', 'Y'),
    ("KeyU", 'u', 'U'),
    ("KeyI", 'i', 'I'),
    ("KeyO", 'o', 'O'),
    ("KeyP", 'p', 'P'),
    ("BracketLeft", '[', '{'),
    ("BracketRight", ']', '}'),
    ("Backslash", '\\', '|'),
    ("KeyA", 'a', 'A'),
    ("KeyS", 's', 'S'),
    ("KeyD", 'd', 'D'),
    ("KeyF", 'f', 'F'),
    ("KeyG", 'g', 'G'),
    ("KeyH", 'h', 'H'),
    ("KeyJ", 'j', 'J'),
    ("KeyK", 'k', 'K'),
    ("KeyL", 'l', 'L'),
    ("Semicolon", ';', ':'),
    ("Quote", '\'', '"'),
    ("KeyZ", 'z', 'Z'),
    ("KeyX", 'x', 'X'),
    ("KeyC", 'c', 'C'),
    ("KeyV", 'v', 'V'),
    ("KeyB", 'b', 'B'),
    ("KeyN", 'n', 'N'),
    ("KeyM", 'm', 'M'),
    ("Comma", ',', '<'),
    ("Period", '.', '>'),
    ("Slash", '/', '?'),
];

const QWERTZ_DE: LayoutTable = &[
    ("Backquote", '^', '°'),
    ("Digit1", '1', '!'),
    ("Digit2", '2', '"'),
    ("Digit3", '3', '§'),
    ("Digit4", '4', '$'),
    ("Digit5", '5', '%'),
    ("Digit6", '6', '&'),
    ("Digit7", '7', '/'),
    ("Digit8", '8', '('),
    ("Digit9", '9', ')'),
    ("Digit0", '0', '='),
    ("Minus", 'ß', '?'),
    ("Equal", '´', '`'),
    ("KeyQ", 'q', 'Q'),
    ("KeyW", 'w', 'W'),
    ("KeyE", 'e', 'E'),
    ("KeyR", 'r', 'R'),
    ("KeyT", 't', 'T'),
    ("KeyY", 'z', 'Z'),
    ("KeyU", 'u', 'U'),
    ("KeyI", 'i', 'I'),
    ("KeyO", 'o', 'O'),
    ("KeyP", 'p', 'P'),
    ("BracketLeft", 'ü', 'Ü'),
    ("BracketRight", '+', '*'),
    ("Backslash", '#', '\''),
    ("KeyA", 'a', 'A'),
    ("KeyS", 's', 'S'),
    ("KeyD", 'd', 'D'),
    ("KeyF", 'f', 'F'),
    ("KeyG", 'g', 'G'),
    ("KeyH", 'h', 'H'),
    ("KeyJ", 'j', 'J'),
    ("KeyK", 'k', 'K'),
    ("KeyL", 'l', 'L'),
    ("Semicolon", 'ö', 'Ö'),
    ("Quote", 'ä', 'Ä'),
    ("IntlBackslash", '<', '>'),
    ("KeyZ", 'y', 'Y'),
    ("KeyX", 'x', 'X'),
    ("KeyC", 'c', 'C'),
    ("KeyV", 'v', 'V'),
    ("KeyB", 'b', 'B'),
    ("KeyN", 'n', 'N'),
    ("KeyM", 'm', 'M'),
    ("Comma", ',', ';'),
    ("Period", '.', ':'),
    ("Slash", '-', '_'),
];

const AZERTY_FR: LayoutTable = &[
    ("Backquote", '²', '\0'),
    ("Digit1", '&', '1'),
    ("Digit2", 'é', '2'),
    ("Digit3", '"', '3'),
    ("Digit4", '\'', '4'),
    ("Digit5", '(', '5'),
    ("Digit6", '-', '6'),
    ("Digit7", 'è', '7'),
    ("Digit8", '_', '8'),
    ("Digit9", 'ç', '9'),
    ("Digit0", 'à', '0'),
    ("Minus", ')', '°'),
    ("Equal", '=', '+'),
    ("KeyQ", 'a', 'A'),
    ("KeyW", 'z', 'Z'),
    ("KeyE", 'e', 'E'),
    ("KeyR", 'r', 'R'),
    ("KeyT", 't', 'T'),
    ("KeyY", 'y', 'Y'),
    ("KeyU", 'u', 'U'),
    ("KeyI", 'i', 'I'),
    ("KeyO", 'o', 'O'),
    ("KeyP", 'p', 'P'),
    ("BracketLeft", '^', '¨'),
    ("BracketRight", '$', '£'),
    ("Backslash", '*', 'µ'),
    ("KeyA", 'q', 'Q'),
    ("KeyS", 's', 'S'),
    ("KeyD", 'd', 'D'),
    ("KeyF", 'f', 'F'),
    ("KeyG", 'g', 'G'),
    ("KeyH", 'h', 'H'),
    ("KeyJ", 'j', 'J'),
    ("KeyK", 'k', 'K'),
    ("KeyL", 'l', 'L'),
    ("Semicolon", 'm', 'M'),
    ("Quote", 'ù', '%'),
    ("IntlBackslash", '<', '>'),
    ("KeyZ", 'w', 'W'),
    ("KeyX", 'x', 'X'),
    ("KeyC", 'c', 'C'),
    ("KeyV", 'v', 'V'),
    ("KeyB", 'b', 'B'),
    ("KeyN", 'n', 'N'),
    ("KeyM", ',', '?'),
    ("Comma", ';', '.'),
    ("Period", ':', '/'),
    ("Slash", '!', '§'),
];

impl Layout {
    /// The layouts the check covers.
    pub const ALL: [Layout; 3] = [Layout::QwertyUs, Layout::QwertzDe, Layout::AzertyFr];

    /// The layout's name (`QWERTY-US`).
    pub fn name(self) -> &'static str {
        match self {
            Layout::QwertyUs => "QWERTY-US",
            Layout::QwertzDe => "QWERTZ-DE",
            Layout::AzertyFr => "AZERTY-FR",
        }
    }

    fn table(self) -> LayoutTable {
        match self {
            Layout::QwertyUs => QWERTY_US,
            Layout::QwertzDe => QWERTZ_DE,
            Layout::AzertyFr => AZERTY_FR,
        }
    }

    /// Where `c` is on the layout: `Some(false)` on a base level, `Some(true)` only on a Shift level, `None` not
    /// without AltGr. Letters are found in either case.
    fn level(self, c: char) -> Option<bool> {
        let c = c.to_ascii_lowercase();
        if self.table().iter().any(|(_, base, _)| *base == c) {
            Some(false)
        } else if self.table().iter().any(|(_, _, shift)| *shift == c) {
            Some(true)
        } else {
            None
        }
    }
}

/// How a binding types on a layout.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Typing {
    /// Typable as written.
    Yes,
    /// Its character needs Shift there and the chord has none (an error).
    NeedsShift,
    /// Its character is not on the layout without AltGr.
    No,
}

/// How `chord` types on `layout`: a physical key and a layout-independent name always; a character by its level.
fn typing(chord: &Chord, layout: Layout) -> Typing {
    let c = match &chord.key {
        ChordKey::Physical(code) => {
            return if layout.table().iter().any(|(k, _, _)| k == code) {
                Typing::Yes
            } else {
                Typing::No
            };
        }
        ChordKey::Logical(name) => match name.as_str() {
            "Plus" => '+',
            "Minus" => '-',
            "Equals" => '=',
            single if single.chars().count() == 1 => single.chars().next().unwrap_or(' '),
            _ => return Typing::Yes,
        },
    };
    match layout.level(c) {
        Some(false) => Typing::Yes,
        // A letter's capital is its Shift level, but a letter binds in either case.
        Some(true) if chord.shift || c.is_ascii_alphabetic() => Typing::Yes,
        Some(true) => Typing::NeedsShift,
        None => Typing::No,
    }
}

/// Whether a finding is an error or an info.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Severity {
    /// The keymap is refused (a user's) or the preset is wrong (a test).
    Error,
    /// Listed in the editor: a context's chord shadows a global one.
    Info,
}

/// A finding of the conflict and reachability rules (D8).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Finding {
    /// Equal chords for different actions in one context (an error).
    Conflict {
        /// The chord.
        chord: Chord,
        /// The context.
        context: Context,
        /// The actions, in registry order.
        actions: Vec<&'static str>,
    },
    /// A context's chord over a global one (an info): `action` shadows `shadowed` in `context`.
    Shadow {
        /// The chord.
        chord: Chord,
        /// The child context.
        context: Context,
        /// The action bound in the child context.
        action: &'static str,
        /// The global action it shadows there.
        shadowed: &'static str,
    },
    /// Ctrl+Alt+any key: AltGr on Windows layouts (an error).
    CtrlAlt {
        /// The chord.
        chord: Chord,
        /// The action.
        action: &'static str,
    },
    /// A chord the system keeps: Alt+F4, Alt+Tab, Win+anything (an error).
    Reserved {
        /// The chord.
        chord: Chord,
        /// The action.
        action: &'static str,
    },
    /// The chord's character needs Shift on `layout` and the chord has none: an error, unless another binding of
    /// the action types on that layout (`covered`), when it is an info (KM-4 ruling, cycle 40).
    NeedsShift {
        /// The chord.
        chord: Chord,
        /// The action.
        action: &'static str,
        /// The layout.
        layout: Layout,
        /// Whether another binding of the action types on the layout.
        covered: bool,
    },
    /// No binding of the action types on `layout` without AltGr (an error).
    Unreachable {
        /// The action.
        action: &'static str,
        /// The layout.
        layout: Layout,
    },
}

impl Finding {
    /// Its severity.
    pub fn severity(&self) -> Severity {
        match self {
            Finding::Shadow { .. } | Finding::NeedsShift { covered: true, .. } => Severity::Info,
            _ => Severity::Error,
        }
    }

    /// Whether it involves `action`.
    pub fn involves(&self, action: &str) -> bool {
        match self {
            Finding::Conflict { actions, .. } => actions.contains(&action),
            Finding::Shadow {
                action: a, shadowed, ..
            } => *a == action || *shadowed == action,
            Finding::CtrlAlt { action: a, .. }
            | Finding::Reserved { action: a, .. }
            | Finding::NeedsShift { action: a, .. }
            | Finding::Unreachable { action: a, .. } => *a == action,
        }
    }

    /// The English text the editor and the banner show.
    pub fn text(&self) -> String {
        let label = |id: &str| {
            crate::actions::find(id).map_or_else(|| id.to_owned(), |m| strings::text(m.label_key).to_owned())
        };
        match self {
            Finding::Conflict {
                chord,
                context,
                actions,
            } => strings::format(
                "keymap-conflict",
                &[
                    &chord.text(),
                    context_word(*context),
                    &actions.iter().map(|a| label(a)).collect::<Vec<_>>().join(", "),
                ],
            ),
            Finding::Shadow {
                chord,
                context,
                shadowed,
                ..
            } => strings::format(
                "keymap-shadow",
                &[&chord.text(), context_word(*context), &label(shadowed)],
            ),
            Finding::CtrlAlt { chord, .. } => strings::format("keymap-ctrl-alt", &[&chord.text()]),
            Finding::Reserved { chord, .. } => strings::format("keymap-reserved", &[&chord.text()]),
            Finding::NeedsShift { chord, layout, .. } => {
                strings::format("keymap-needs-shift", &[&chord.text(), layout.name()])
            }
            Finding::Unreachable { action, layout } => {
                strings::format("keymap-unreachable", &[&label(action), layout.name()])
            }
        }
    }
}

/// Whether the system keeps `chord`: Alt+F4, Alt+Tab, anything with Win.
fn reserved(chord: &Chord) -> bool {
    let key = match &chord.key {
        ChordKey::Logical(k) => k.as_str(),
        ChordKey::Physical(_) => "",
    };
    chord.win || (chord.alt && !chord.ctrl && !chord.shift && matches!(key, "F4" | "Tab"))
}

/// The conflicts, the shadowing, the Ctrl+Alt ban and the reserved chords of `binds` (D8, doc 16 §15.5), in the
/// order of the binds.
pub fn conflicts(binds: &[Bound]) -> Vec<Finding> {
    let mut out = Vec::new();
    for (i, b) in binds.iter().enumerate() {
        if b.chord.ctrl && b.chord.alt {
            out.push(Finding::CtrlAlt {
                chord: b.chord.clone(),
                action: b.action,
            });
        }
        if reserved(&b.chord) {
            out.push(Finding::Reserved {
                chord: b.chord.clone(),
                action: b.action,
            });
        }
        // One conflict per chord and context, reported at its first bind.
        let same = |o: &Bound| o.chord == b.chord && o.context == b.context;
        if !binds[..i].iter().any(&same) {
            let mut actions: Vec<&'static str> = Vec::new();
            for o in binds[i..].iter().filter(|o| same(o)) {
                if !actions.contains(&o.action) {
                    actions.push(o.action);
                }
            }
            if actions.len() > 1 {
                out.push(Finding::Conflict {
                    chord: b.chord.clone(),
                    context: b.context,
                    actions,
                });
            }
        }
        if b.context != Context::Global
            && let Some(g) = binds
                .iter()
                .find(|o| o.context == Context::Global && o.chord == b.chord && o.action != b.action)
        {
            out.push(Finding::Shadow {
                chord: b.chord.clone(),
                context: b.context,
                action: b.action,
                shadowed: g.action,
            });
        }
    }
    out
}

/// The reachability findings of `binds` on each layout (D8): a binding whose character needs a Shift it does not
/// have (an info when another binding of the action types there), and an action with no typable binding.
pub fn reachability(binds: &[Bound]) -> Vec<Finding> {
    let mut out = Vec::new();
    for layout in Layout::ALL {
        let mut actions: Vec<&'static str> = Vec::new();
        for b in binds {
            if !actions.contains(&b.action) {
                actions.push(b.action);
            }
            if typing(&b.chord, layout) == Typing::NeedsShift {
                let covered = binds
                    .iter()
                    .any(|o| o.action == b.action && typing(&o.chord, layout) == Typing::Yes);
                out.push(Finding::NeedsShift {
                    chord: b.chord.clone(),
                    action: b.action,
                    layout,
                    covered,
                });
            }
        }
        for action in actions {
            if !binds
                .iter()
                .any(|b| b.action == action && typing(&b.chord, layout) == Typing::Yes)
            {
                out.push(Finding::Unreachable { action, layout });
            }
        }
    }
    out
}

/// The errors that refuse the user's keymap `user` over `preset` (D8): the conflicts, the Ctrl+Alt ban and the
/// reserved chords of the effective bindings. Reachability is a preset rule, not a user's.
pub fn user_errors(preset: Preset, user: &Keymap) -> Vec<Finding> {
    conflicts(&effective(preset, Some(user)))
        .into_iter()
        .filter(|f| f.severity() == Severity::Error)
        .collect()
}

// ---------------------------------------------------------------------------------------------------------------
// The keymap sheet (KM-6)

/// The keymap sheet `keymaps/sheet.json` (NFR-DOC-06): per action with a binding in a preset, its ID, label key,
/// English label, context and the bindings per preset, in registry order. `docs-gen` writes the book's keymap page
/// from it.
pub fn sheet_json() -> String {
    let mut actions = Vec::new();
    for meta in ACTIONS {
        let per: Vec<(Preset, Vec<String>)> = Preset::ALL
            .iter()
            .map(|p| {
                (
                    *p,
                    chords_of(*p, None, meta)
                        .into_iter()
                        .map(|(c, _)| c.text())
                        .collect(),
                )
            })
            .collect();
        if per.iter().all(|(_, b)| b.is_empty()) {
            continue;
        }
        let mut bindings = serde_json::Map::new();
        for (p, b) in per {
            bindings.insert(p.word().to_owned(), serde_json::json!(b));
        }
        actions.push(serde_json::json!({
            "action": meta.id,
            "label_key": meta.label_key,
            "label": strings::text(meta.label_key),
            "context": context_word(meta.context),
            "bindings": bindings,
        }));
    }
    let presets: Vec<serde_json::Value> = Preset::ALL
        .iter()
        .map(|p| serde_json::json!({ "id": p.word(), "name": p.name() }))
        .collect();
    let mut text = serde_json::to_string_pretty(&serde_json::json!({
        "presets": presets,
        "actions": actions,
    }))
    .unwrap_or_default();
    text.push('\n');
    text
}

// ---------------------------------------------------------------------------------------------------------------
// The editor

/// A refused edit of the keymap editor: what was tried and the errors it would have made.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Refusal {
    /// The action edited.
    pub action: &'static str,
    /// The chord recorded, as text; empty for a reset.
    pub chord: String,
    /// The errors, as the editor lists them.
    pub errors: Vec<String>,
}

/// The state of the keymap editor (Help ▸ Keyboard shortcuts, D8): transient, reset when the dialog opens.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct EditorState {
    /// The search text.
    pub search: String,
    /// The action whose next chord the window records.
    pub recording: Option<&'static str>,
    /// The last refused edit.
    pub refusal: Option<Refusal>,
}

/// One row of the editor: a bindable action (not hidden, not headless-only) with its effective bindings.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EditorRow {
    /// The action.
    pub action: &'static str,
    /// Its label.
    pub label: String,
    /// The context of its bindings (the registry's when it has none).
    pub context: Context,
    /// The bindings, as text.
    pub chords: Vec<String>,
    /// Whether they differ from the preset's (W40a 5b: a user bind equal to the preset's is no change).
    pub custom: bool,
    /// The shadowing texts that involve the action (infos).
    pub notes: Vec<String>,
}

/// Whether `row` matches the search `query`: every word of it in the label, the ID or a binding, ignoring case.
pub fn row_matches(row: &EditorRow, query: &str) -> bool {
    let hay = format!(
        "{} {} {}",
        row.label.to_lowercase(),
        row.action,
        row.chords.join(" ").to_lowercase()
    );
    query.to_lowercase().split_whitespace().all(|w| hay.contains(w))
}

/// The editor's rows under `preset` and `user`, in registry order, filtered by `query`.
pub fn editor_rows(preset: Preset, user: Option<&Keymap>, query: &str) -> Vec<EditorRow> {
    let all = effective(preset, user);
    let findings = conflicts(&all);
    ACTIONS
        .iter()
        .filter(|m| !m.hidden && !m.headless_only)
        .filter_map(|meta| {
            let chords = chords_of(preset, user, meta);
            let row = EditorRow {
                action: meta.id,
                label: strings::text(meta.label_key).to_owned(),
                context: chords.first().map_or(meta.context, |(_, c)| *c),
                chords: chords.iter().map(|(c, _)| c.text()).collect(),
                custom: is_changed(preset, user, meta),
                notes: findings
                    .iter()
                    .filter(|f| matches!(f, Finding::Shadow { action, .. } if *action == meta.id))
                    .map(Finding::text)
                    .collect(),
            };
            row_matches(&row, query).then_some(row)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chord(text: &str) -> Chord {
        Chord::parse(text).unwrap()
    }

    #[test]
    fn every_registry_binding_parses_and_prints_as_written() {
        for meta in ACTIONS {
            for s in meta.shortcut {
                assert_eq!(chord(s).text(), *s, "{}", meta.id);
            }
        }
    }

    #[test]
    fn chords_normalise() {
        assert_eq!(chord("shift+ctrl+z"), chord("Ctrl+Shift+Z"));
        assert_eq!(chord("Ctrl+Shift+z").text(), "Ctrl+Shift+Z");
        // A punctuation character is its physical key; the file writes the `phys:` form.
        assert_eq!(chord("]"), chord("phys:BracketRight"));
        assert_eq!(chord("]").file_text(), "phys:BracketRight");
        assert_eq!(chord("phys:BracketRight").text(), "]");
        assert_eq!(chord("Ctrl+phys:Digit0").text(), "Ctrl+phys:Digit0");
        assert_eq!(chord("page down").text(), "Page Down");
        assert!(Chord::parse("Ctrl+NoSuchKey").is_err());
        assert!(Chord::parse("Hyper+A").is_err());
        assert!(Chord::parse("Ctrl+Ctrl+A").is_err());
        assert!(Chord::parse("phys:IntlRo").is_err());
        assert!(Chord::parse("").is_err());
    }

    #[test]
    fn a_keymap_file_round_trips() {
        for p in Preset::ALL {
            let k = p.keymap();
            assert_eq!(Keymap::parse(&k.to_toml()).unwrap(), k);
        }
        let text = "[meta]\nname = \"Mine\"\nversion = 1\n\n[[bind]]\nkeys = \"phys:BracketRight\"\naction = \"cp.next\"\ncontext = \"canvas\"\nalt = \"Alt+N\"\n";
        let k = Keymap::parse(text).unwrap();
        assert_eq!(k.binds[0].keys, chord("]"));
        assert_eq!(k.binds[0].alt, Some(chord("Alt+N")));
        assert_eq!(k.to_toml(), text);
    }

    #[test]
    fn a_bad_keymap_file_is_refused_with_a_reason() {
        assert!(Keymap::parse("not toml [").is_err());
        assert!(Keymap::parse("[meta]\nversion = 2\n").is_err());
        assert!(Keymap::parse("[[bind]]\nkeys = \"W\"\n").is_err());
        let e = Keymap::parse(
            "[meta]\nversion = 1\n[[bind]]\nkeys = \"W\"\naction = \"tool.wire\"\ncontext = \"tool\"\n",
        )
        .unwrap_err();
        assert!(e.contains("tool"), "{e}");
        let e = Keymap::parse("[meta]\nversion = 1\n[[bind]]\nkeys = \"Ctrl+Nope\"\naction = \"tool.wire\"\ncontext = \"canvas\"\n")
            .unwrap_err();
        assert!(e.contains("Ctrl+Nope"), "{e}");
    }

    #[test]
    fn the_kicad_preset_differs_from_the_ew_preset_only_in_place_and_move() {
        for meta in ACTIONS {
            let (ew, kicad) = (Preset::Ew.shortcuts(meta), Preset::Kicad.shortcuts(meta));
            match meta.id {
                "tool.place" => assert_eq!((ew, kicad), (&["I"][..], &["A"][..])),
                "tool.move" => assert_eq!((ew, kicad), (&["M"][..], &["M", "G"][..])),
                _ => assert_eq!(ew, kicad, "{}", meta.id),
            }
        }
    }

    fn bound(action: &'static str, keys: &str, context: Context) -> Bound {
        Bound {
            action,
            chord: chord(keys),
            context,
        }
    }

    #[test]
    fn conflicts_shadowing_and_the_banned_chords() {
        let binds = [
            bound("a.one", "Ctrl+K", Context::Global),
            bound("a.two", "Ctrl+K", Context::Global),
            bound("a.three", "Ctrl+K", Context::Canvas),
            bound("a.four", "Ctrl+Alt+1", Context::Global),
            bound("a.five", "Alt+F4", Context::Global),
            bound("a.six", "Win+E", Context::Canvas),
            bound("a.seven", "Ctrl+Shift+K", Context::Find),
            bound("a.seven", "Ctrl+Shift+K", Context::Find),
        ];
        let f = conflicts(&binds);
        assert_eq!(
            f,
            [
                Finding::Conflict {
                    chord: chord("Ctrl+K"),
                    context: Context::Global,
                    actions: vec!["a.one", "a.two"],
                },
                Finding::Shadow {
                    chord: chord("Ctrl+K"),
                    context: Context::Canvas,
                    action: "a.three",
                    shadowed: "a.one",
                },
                Finding::CtrlAlt {
                    chord: chord("Ctrl+Alt+1"),
                    action: "a.four",
                },
                Finding::Reserved {
                    chord: chord("Alt+F4"),
                    action: "a.five",
                },
                Finding::Reserved {
                    chord: chord("Win+E"),
                    action: "a.six",
                },
            ]
        );
        assert_eq!(f[1].severity(), Severity::Info);
        assert!(f.iter().filter(|x| x.severity() == Severity::Error).count() == 4);
        // Siblings never conflict: the canvas and the find pane are never active together.
        assert!(conflicts(&[bound("a", "F2", Context::Canvas), bound("b", "F2", Context::Find)]).is_empty());
    }

    #[test]
    fn reachability_per_layout() {
        // A digit needs Shift on AZERTY; the physical key does not.
        let f = reachability(&[bound("a.zero", "Ctrl+0", Context::Global)]);
        assert_eq!(
            f,
            [
                Finding::NeedsShift {
                    chord: chord("Ctrl+0"),
                    action: "a.zero",
                    layout: Layout::AzertyFr,
                    covered: false,
                },
                Finding::Unreachable {
                    action: "a.zero",
                    layout: Layout::AzertyFr,
                },
            ]
        );
        assert!(reachability(&[bound("a.zero", "Ctrl+phys:Digit0", Context::Global)]).is_empty());
        assert!(reachability(&[bound("a.zero", "Ctrl+Shift+0", Context::Global)]).is_empty());
        // Brackets by physical key, letters and named keys type everywhere.
        assert!(
            reachability(&[
                bound("a", "]", Context::Canvas),
                bound("b", "Shift+W", Context::Canvas),
                bound("c", "Page Down", Context::Global),
            ])
            .is_empty()
        );
        // `=` is Shift+0 on QWERTZ; `+` is Shift+= on QWERTY and AZERTY.
        let f = reachability(&[bound("z", "Ctrl+Equals", Context::Global)]);
        assert_eq!(f.len(), 2, "{f:?}");
        assert!(f.contains(&Finding::Unreachable {
            action: "z",
            layout: Layout::QwertzDe
        }));
        // KM-4 ruling: a binding that needs Shift is an info when another binding of the action types there.
        let f = reachability(&[
            bound("in", "Ctrl+Plus", Context::Global),
            bound("in", "Ctrl+Equals", Context::Global),
        ]);
        assert!(!f.is_empty());
        assert!(f.iter().all(|x| x.severity() == Severity::Info), "{f:?}");
        assert!(f.iter().all(|x| matches!(x, Finding::NeedsShift { covered: true, .. })));
        let lone = reachability(&[bound("a.zero", "Ctrl+0", Context::Global)]);
        assert_eq!(lone[0].severity(), Severity::Error);
    }

    #[test]
    fn the_user_keymap_replaces_the_bindings_of_the_actions_it_names() {
        let user = Keymap::parse(
            "[meta]\nname = \"Mine\"\nversion = 1\n[[bind]]\nkeys = \"Ctrl+Shift+W\"\naction = \"tool.wire\"\ncontext = \"canvas\"\n\
             [[bind]]\nkeys = \"Ctrl+J\"\naction = \"future.action\"\ncontext = \"global\"\n",
        )
        .unwrap();
        let wire = crate::actions::find("tool.wire").unwrap();
        assert_eq!(
            chords_of(Preset::Ew, Some(&user), wire),
            [(chord("Ctrl+Shift+W"), Context::Canvas)]
        );
        assert_eq!(user.unused().len(), 1);
        assert_eq!(user.unused()[0].action, "future.action");
        // Ctrl+Shift+W in the canvas shadows Close project; that is an info, not an error.
        assert!(user_errors(Preset::Ew, &user).is_empty());
        let rows = editor_rows(Preset::Ew, Some(&user), "wire");
        let row = rows.iter().find(|r| r.action == "tool.wire").unwrap();
        assert!(row.custom);
        assert_eq!(row.chords, ["Ctrl+Shift+W"]);
        assert_eq!(row.notes.len(), 1, "{:?}", row.notes);
        assert!(has_changes(Preset::Ew, Some(&user)));
    }

    /// W40a 5b: a user keymap whose bindings equal the preset's changes nothing: no row is Changed and the preset line
    /// has no changes; one different binding makes exactly that action Changed.
    #[test]
    fn a_user_keymap_equal_to_the_preset_changes_nothing() {
        let same = export("EW (changed)", Preset::Ew, None);
        assert!(!has_changes(Preset::Ew, Some(&same)));
        assert!(editor_rows(Preset::Ew, Some(&same), "").iter().all(|r| !r.custom));
        let mut one = same.clone();
        let wire = one.binds.iter_mut().find(|b| b.action == "tool.wire").unwrap();
        wire.keys = chord("Ctrl+Shift+W");
        wire.alt = None;
        assert!(has_changes(Preset::Ew, Some(&one)));
        let changed: Vec<&str> = editor_rows(Preset::Ew, Some(&one), "")
            .iter()
            .filter(|r| r.custom)
            .map(|r| r.action)
            .collect();
        assert_eq!(changed, ["tool.wire"]);
    }

    /// W40a 5c: a refused recording says what is in the way; a loaded file's double binding keeps the conflict text.
    #[test]
    fn a_refused_recording_names_the_action_in_the_way() {
        let conflict = Finding::Conflict {
            chord: chord("M"),
            context: Context::Canvas,
            actions: vec!["tool.move", "tool.wire"],
        };
        let wire = strings::text(crate::actions::find("tool.wire").unwrap().label_key);
        let r#move = strings::text(crate::actions::find("tool.move").unwrap().label_key);
        assert_eq!(
            refusal_text(&conflict, "tool.wire"),
            format!("M is already {move} in the canvas.")
        );
        assert_eq!(
            conflict.text(),
            format!("M is bound twice in the canvas context: {move}, {wire}")
        );
    }
}
