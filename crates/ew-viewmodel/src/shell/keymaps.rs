// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The keymaps in the shell (WP-S2.08 part 2, D6 to D8; the files and the rules in [`crate::keymap`]): the active
//! preset (the `keymap` setting) with the user's `keymap.toml` over it, the effective bindings the window binds and the
//! menus, the palette and the describe text show, and the keymap editor's actions (search, record, reset, import,
//! export). Every edit is checked by D8's rules first; an edit with an error is refused and listed in the editor, so
//! the effective keymap never holds an error the user made.

use std::path::Path;

use serde_json::{Map, Value};

use super::{Dialog, Outcome, Prompt, ShellModel, failed, string_param};
use crate::actions::{self, ActionMeta};
use crate::keymap::{self, Bind, Bound, Chord, EditorRow, Keymap, Preset, Refusal};
use crate::strings;

/// Why the user's keymap does not apply at start-up ([`load_user_keymap`]): the banner text, and whether the editor
/// must leave `keymap.toml` as it is (it could not be read, or a refused file could not be copied aside).
#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) struct KeymapTrouble {
    /// The banner.
    pub banner: String,
    /// Whether writing `keymap.toml` is blocked for this run.
    pub keep_file: bool,
}

/// A file reason (a sentence of [`Keymap::parse`], which starts with an English word, never with a chord) as the clause
/// after a colon: its first letter in lower case ("it is not valid TOML.").
fn clause(reason: &str) -> String {
    let mut chars = reason.chars();
    chars
        .next()
        .map_or_else(String::new, |first| first.to_lowercase().chain(chars).collect())
}

/// The user's keymap in `dir` checked against the preset `preset_word`: `Ok(None)` without a file. A file that does
/// not parse or has an error (D8) is copied to `keymap.toml.bad-<n>` (once: a copy with the same bytes is named
/// again, FIX40d) and refused with a banner; a file that cannot be
/// read, or a refused one whose copy-aside failed, is refused with a banner and kept: the editor does not write over
/// it in this run (C40 review part 2, finding 8).
pub(super) fn load_user_keymap(dir: &Path, preset_word: &str) -> Result<Option<Keymap>, KeymapTrouble> {
    let preset = Preset::from_setting(preset_word);
    let reason = match keymap::read_user(dir) {
        Ok(None) => return Ok(None),
        Ok(Some(k)) => {
            let errors = keymap::user_errors(preset, &k);
            if errors.is_empty() {
                return Ok(Some(k));
            }
            errors
                .iter()
                .map(keymap::Finding::text)
                .collect::<Vec<_>>()
                .join("; ")
        }
        Err(keymap::UserFileError::Read(error)) => {
            return Err(KeymapTrouble {
                banner: strings::format("banner-keymap-unread", &[&error, preset.name()]),
                keep_file: true,
            });
        }
        Err(keymap::UserFileError::Invalid(reason)) => clause(&reason),
    };
    let reason = reason.trim_end_matches('.');
    Err(
        match crate::settings::copy_aside_once_as(&dir.join(keymap::USER_FILE), keymap::USER_FILE) {
            Some(copy) => KeymapTrouble {
                banner: strings::format("banner-keymap-refused", &[reason, preset.name(), &copy]),
                keep_file: false,
            },
            None => KeymapTrouble {
                banner: strings::format("banner-keymap-refused-kept", &[reason, preset.name()]),
                keep_file: true,
            },
        },
    )
}

impl ShellModel {
    /// The active keymap preset (the `keymap` setting).
    pub fn keymap_preset(&self) -> Preset {
        Preset::from_setting(&self.settings.keymap)
    }

    /// The user's keymap, when there is one.
    pub fn user_keymap(&self) -> Option<&Keymap> {
        self.user_keymap.as_ref()
    }

    /// Bumped whenever the user's keymap changes (the window rebuilds its bindings when this or the preset changes).
    pub fn keymap_version(&self) -> u64 {
        self.keymap_version
    }

    /// The effective bindings, in registry order (D6).
    pub fn bindings(&self) -> Vec<Bound> {
        keymap::effective(self.keymap_preset(), self.user_keymap.as_ref())
    }

    /// The effective binding texts of `meta`, as the menus and the palette show them.
    pub fn shortcuts_of(&self, meta: &ActionMeta) -> Vec<String> {
        keymap::chords_of(self.keymap_preset(), self.user_keymap.as_ref(), meta)
            .into_iter()
            .map(|(c, _)| c.text())
            .collect()
    }

    /// The first effective binding text of `meta`, empty when it has none.
    pub fn shortcut_text(&self, meta: &ActionMeta) -> String {
        self.shortcuts_of(meta).into_iter().next().unwrap_or_default()
    }

    /// The keymap editor's state.
    pub fn keymap_editor(&self) -> &keymap::EditorState {
        &self.keymap_editor
    }

    /// The keymap editor's rows for its search.
    pub fn keymap_rows(&self) -> Vec<EditorRow> {
        keymap::editor_rows(
            self.keymap_preset(),
            self.user_keymap.as_ref(),
            &self.keymap_editor.search,
        )
    }

    /// `keymap.search {text}`: the editor's search text.
    pub(super) fn act_keymap_search(&mut self, p: &Map<String, Value>) -> Outcome {
        match string_param(p, "text") {
            Err(e) => e,
            Ok(text) => {
                self.keymap_editor.search = text.unwrap_or_default().to_owned();
                Outcome::Done
            }
        }
    }

    /// The editable action named by parameter `action` (not hidden, not headless-only).
    fn keymap_action(p: &Map<String, Value>) -> Result<&'static ActionMeta, Outcome> {
        let id = string_param(p, "action")?.unwrap_or_default();
        actions::find(id)
            .filter(|m| !m.hidden && !m.headless_only)
            .ok_or_else(|| failed("error-keymap-action", &[id]))
    }

    /// `keymap.record_start {action?}`: the window records the next chord for `action`; without it the recording
    /// ends.
    pub(super) fn act_keymap_record_start(&mut self, p: &Map<String, Value>) -> Outcome {
        if !p.contains_key("action") {
            self.keymap_editor.recording = None;
            return Outcome::Done;
        }
        match Self::keymap_action(p) {
            Err(e) => e,
            Ok(meta) => {
                self.keymap_editor.recording = Some(meta.id);
                self.keymap_editor.refusal = None;
                Outcome::Done
            }
        }
    }

    /// `keymap.record {action, keys}`: `keys` becomes the action's one binding in the user's keymap, in the action's
    /// context. Refused (and listed in the editor) when the result has an error of D8.
    pub(super) fn act_keymap_record(&mut self, p: &Map<String, Value>) -> Outcome {
        let meta = match Self::keymap_action(p) {
            Err(e) => return e,
            Ok(m) => m,
        };
        let keys = match string_param(p, "keys") {
            Err(e) => return e,
            Ok(k) => k.unwrap_or_default(),
        };
        self.keymap_editor.recording = None;
        let chord = match Chord::parse(keys) {
            Ok(c) => c,
            Err(reason) => {
                self.keymap_editor.refusal = Some(Refusal {
                    action: meta.id,
                    chord: keys.to_owned(),
                    errors: vec![reason],
                });
                return Outcome::Refused {
                    reason_key: "reason-keymap-conflict",
                };
            }
        };
        let mut user = self.user_keymap.clone().unwrap_or_else(|| Keymap {
            name: strings::format("keymap-custom-name", &[self.keymap_preset().name()]),
            binds: Vec::new(),
        });
        user.binds.retain(|b| b.action != meta.id);
        user.binds.push(Bind {
            keys: chord.clone(),
            action: meta.id.to_owned(),
            context: meta.context,
            alt: None,
        });
        self.apply_user_keymap(meta.id, &chord.text(), user)
    }

    /// `keymap.reset {action}`: the action's bindings return to the preset's.
    pub(super) fn act_keymap_reset(&mut self, p: &Map<String, Value>) -> Outcome {
        let meta = match Self::keymap_action(p) {
            Err(e) => return e,
            Ok(m) => m,
        };
        self.keymap_editor.recording = None;
        let Some(mut user) = self.user_keymap.clone() else {
            self.keymap_editor.refusal = None;
            return Outcome::Done;
        };
        user.binds.retain(|b| b.action != meta.id);
        self.apply_user_keymap(meta.id, "", user)
    }

    /// Makes `user` the user's keymap when it has no error (and writes it), else lists the refusal of the edit of
    /// `action` with `chord`.
    fn apply_user_keymap(&mut self, action: &'static str, chord: &str, user: Keymap) -> Outcome {
        let errors = keymap::user_errors(self.keymap_preset(), &user);
        if !errors.is_empty() {
            self.keymap_editor.refusal = Some(Refusal {
                action,
                chord: chord.to_owned(),
                errors: errors.iter().map(|f| keymap::refusal_text(f, action)).collect(),
            });
            return Outcome::Refused {
                reason_key: "reason-keymap-conflict",
            };
        }
        self.keymap_editor.refusal = None;
        self.set_user_keymap(user);
        Outcome::Done
    }

    /// Installs `user` as the user's keymap and writes `keymap.toml` (a banner when the write fails). While the file
    /// must be kept (it could not be read or copied aside at start-up), the keymap applies for this run only and the
    /// file is left as it is, with a banner.
    fn set_user_keymap(&mut self, user: Keymap) {
        if let Some(dir) = &self.config.config_dir {
            if self.keymap_keep_file {
                self.banner = Some(strings::text("banner-keymap-kept").to_owned());
            } else if keymap::write_user(dir, &user).is_err() {
                self.banner = Some(strings::text("banner-keymap-write").to_owned());
            }
        }
        self.user_keymap = Some(user);
        self.keymap_version += 1;
    }

    /// `keymap.import {path?}`: the keymap file at `path` becomes the user's keymap, when it parses and has no error
    /// of D8 over the active preset; without `path` the window asks for the file.
    pub(super) fn act_keymap_import(&mut self, p: &Map<String, Value>) -> Outcome {
        let path = match string_param(p, "path") {
            Err(e) => return e,
            Ok(None) => return self.need_path("keymap.import", Prompt::OpenKeymap, "path", p),
            Ok(Some(path)) => path.to_owned(),
        };
        let parsed = std::fs::read(&path)
            .map_err(|_| strings::text("keymap-error-toml").to_owned())
            .and_then(|b| String::from_utf8(b).map_err(|_| strings::text("keymap-error-toml").to_owned()))
            .and_then(|t| Keymap::parse(&t));
        let user = match parsed {
            Ok(k) => k,
            Err(reason) => return failed("error-keymap-import", &[&clause(&reason)]),
        };
        let errors = keymap::user_errors(self.keymap_preset(), &user);
        if !errors.is_empty() {
            let list = errors
                .iter()
                .map(keymap::Finding::text)
                .collect::<Vec<_>>()
                .join("; ");
            return failed("error-keymap-import", &[&list]);
        }
        self.keymap_editor.refusal = None;
        self.set_user_keymap(user);
        Outcome::Done
    }

    /// `keymap.export {path?}`: writes the effective bindings as a self-contained keymap file at `path` (the user's
    /// unused binds included), which imports back equal; without `path` the window asks for the file name.
    pub(super) fn act_keymap_export(&mut self, p: &Map<String, Value>) -> Outcome {
        let path = match string_param(p, "path") {
            Err(e) => return e,
            Ok(None) => return self.need_path("keymap.export", Prompt::SaveKeymap, "path", p),
            Ok(Some(path)) => path.to_owned(),
        };
        let preset = self.keymap_preset();
        let name = match &self.user_keymap {
            Some(_) => strings::format("keymap-custom-name", &[preset.name()]),
            None => preset.name().to_owned(),
        };
        let file = keymap::export(&name, preset, self.user_keymap.as_ref());
        match std::fs::write(&path, file.to_toml()) {
            Ok(()) => Outcome::Done,
            Err(_) => failed("error-keymap-export", &[]),
        }
    }

    /// Whether the keymap editor is open (the window routes key presses to its recording).
    pub fn keymap_editor_open(&self) -> bool {
        matches!(self.dialog, Some(Dialog::Shortcuts))
    }
}
