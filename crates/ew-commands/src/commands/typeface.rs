// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `project.typeface.set` (`SetTypeface`): the project's typeface (doc 09 §4.8), a font file of the user's that every
//! text of the Standard typeface prints in, held as a project asset (`assets/<sha256>.ttf` or `.otf`, doc 05 §13.3).

use ew_schema::{Extra, MAX_TYPEFACE_BYTES, TypefaceFile};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use skrifa::MetadataProvider;
use skrifa::raw::TableProvider;

use crate::command::{Command, Outcome};
use crate::error::CmdError;
use crate::meta::CommandMeta;
use crate::tx::TxBuilder;
use ew_l10n::Text;

/// The longest file name the setting keeps, in characters.
pub const MAX_TYPEFACE_NAME: usize = 255;

/// `project.typeface.set` (`SetTypeface`): sets or removes the project's typeface (doc 09 §4.8).
///
/// - `file`: the font file in base64 (RFC 4648 §4: standard alphabet, padded, no line breaks), a TrueType or OpenType
///   file of one typeface of at most 2.5 MiB ([`MAX_TYPEFACE_BYTES`]), as a list of parts joined in order before
///   decoding: a part is one parameter string of at most 1 MiB, so a file over 768 KiB comes in two to four parts and
///   stays one command, one undo step and one line of a command log. The bytes join the project's asset store and
///   `settings.typeface` names them with `name`. An empty list removes the setting (the asset stays in the store while
///   the session lasts, so undo brings it back).
/// - `name`: the file's name without its folder (`LiberationSans-Regular.ttf`), at most 255 characters; needed with a
///   file.
///
/// Refused at `/file`, in words: parts over the limit together, or that are not base64; bytes that are no font of one typeface (a
/// collection `.ttc`, a web font `.woff`, any other file); a font whose tables cannot be read or that names no family;
/// a font without the letters A to Z, a to z and the digits 0 to 9; a font whose embedding permission (`OS/2.fsType`)
/// is restricted, allows bitmaps only or forbids a subset, since the PDF embeds the subset of the glyphs it prints.
/// Refused at `/name`: an empty name, one with a folder in it, a control character or more than 255 characters.
///
/// Only the project record changes. Bulk: every text of the Standard typeface prints anew, so front-ends preview it.
/// Undoable as one step. Outcome: none.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetTypeface {
    /// The font file (TrueType or OpenType, one typeface, at most 2.5 MiB) in base64, in parts of at most 1 MiB each
    /// joined in order; an empty list removes the typeface.
    pub file: Vec<String>,
    /// The file's name without its folder; needed with a file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl Command for SetTypeface {
    const NAME: &'static str = "project.typeface.set";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME).bulk()
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let typeface = if self.file.is_empty() {
            None
        } else {
            // The parameters first: their errors come before any reading of the project.
            let name = self.name.as_deref().map_or("", str::trim);
            if let Some(why) = name_refusal(name) {
                return Err(CmdError::invalid_params(Self::NAME, "/name", why));
            }
            let refused = |why: Text| CmdError::invalid_params(Self::NAME, "/file", why);
            let too_large = || {
                refused(text!(
                    "refusal-typeface-too-large",
                    (MAX_TYPEFACE_BYTES / 1024).to_string()
                ))
            };
            if self.file.iter().map(String::len).sum::<usize>() > MAX_TYPEFACE_BYTES.div_ceil(3) * 4 {
                return Err(too_large());
            }
            let bytes = ew_schema::base64_decode(&self.file.concat())
                .map_err(|e| refused(text!("refusal-params-detail", crate::error::asset_error_text(&e))))?;
            if bytes.len() > MAX_TYPEFACE_BYTES {
                return Err(too_large());
            }
            if let Some(why) = font_refusal(&bytes) {
                return Err(refused(why));
            }
            let reference = tx
                .put_asset(bytes)
                .map_err(|e| refused(text!("refusal-params-detail", crate::error::asset_error_text(&e))))?;
            Some(TypefaceFile {
                asset: reference.sha256,
                name: name.to_owned(),
                extra: Extra::new(),
            })
        };
        let mut project = (*tx.project()).clone();
        project.settings.typeface = typeface;
        tx.set_project(project)?;
        Ok(Outcome::default())
    }
}

/// Why `name` is no file name the setting keeps; `None` when it is one.
fn name_refusal(name: &str) -> Option<Text> {
    if name.is_empty() {
        return Some(text!("refusal-typeface-name-missing"));
    }
    if name.chars().count() > MAX_TYPEFACE_NAME
        || name
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '\\' | ':'))
    {
        return Some(text!(
            "refusal-typeface-name",
            name.chars().take(80).collect::<String>(),
            MAX_TYPEFACE_NAME.to_string()
        ));
    }
    None
}

/// The characters every typeface must have: the letters and digits a drawing prints.
fn basic_latin() -> impl Iterator<Item = char> {
    ('A'..='Z').chain('a'..='z').chain('0'..='9')
}

/// Why `bytes` are no font the project takes as its typeface (doc 09 §4.8); `None` when they are one.
fn font_refusal(bytes: &[u8]) -> Option<Text> {
    if !ew_schema::sniff(bytes).is_ok_and(|(format, _)| format.is_font()) {
        return Some(text!("refusal-typeface-not-a-font"));
    }
    let Ok(font) = skrifa::FontRef::new(bytes) else {
        return Some(text!("refusal-typeface-damaged"));
    };
    let named = [
        skrifa::raw::types::NameId::FAMILY_NAME,
        skrifa::raw::types::NameId::TYPOGRAPHIC_FAMILY_NAME,
    ]
    .into_iter()
    .any(|id| font.localized_strings(id).english_or_first().is_some());
    if !named || font.head().map_or(true, |h| h.units_per_em() == 0) {
        return Some(text!("refusal-typeface-damaged"));
    }
    // The embedding permission as the OpenType specification reads it: the permission field (bits 0 to 3) is
    // restricted only when it is 2, and bits 8 and 9 forbid a subset and outlines.
    let fs_type = font.os2().map_or(0, |os2| os2.fs_type());
    if fs_type & 0x000F == 0x0002 {
        return Some(text!("refusal-typeface-restricted"));
    }
    if fs_type & 0x0200 != 0 {
        return Some(text!("refusal-typeface-bitmaps-only"));
    }
    if fs_type & 0x0100 != 0 {
        return Some(text!("refusal-typeface-no-subset"));
    }
    let charmap = font.charmap();
    if basic_latin().any(|c| charmap.map(c).is_none()) {
        return Some(text!("refusal-typeface-no-letters"));
    }
    None
}
