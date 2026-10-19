// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Font bytes by [`FontKey`]: how renderers and writers get the files behind the glyph runs of a display
//! list without depending on the text stack (ARCH §3: `ew-render-cpu` and `ew-export` see `ew-dl` only).

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use ew_dl::FontKey;

/// The bytes of one font file and the face index inside it.
#[derive(Clone, PartialEq, Eq)]
pub struct FontBytes {
    /// The whole font file.
    pub data: Arc<[u8]>,
    /// The face index in a collection; 0 for a single-face file.
    pub index: u32,
}

impl fmt::Debug for FontBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FontBytes")
            .field("len", &self.data.len())
            .field("index", &self.index)
            .finish()
    }
}

/// Resolves the [`FontKey`] of a glyph run to its font file. The caller fills it from the font set the
/// runs were shaped with (bundled or project-embedded files only, never system fonts; NFR-REL-11).
///
/// `None` means the font is unavailable: renderers then skip the run's glyphs and report it, never
/// substituting another font.
pub trait FontResolver {
    /// The file behind `key`.
    fn font(&self, key: FontKey) -> Option<FontBytes>;
}

impl FontResolver for BTreeMap<FontKey, FontBytes> {
    fn font(&self, key: FontKey) -> Option<FontBytes> {
        self.get(&key).cloned()
    }
}
