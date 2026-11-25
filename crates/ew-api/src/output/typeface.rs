// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The project's typeface (doc 09 §4.8): the font set every page layer, the panel labels and the export shape the
//! project's texts with ([`ProjectFontsQ`], [`fonts`]), and the fonts the writers and renderers draw them with
//! ([`DrawingFonts`]).
//!
//! The set is the bundled one, or with `settings.typeface` the set of `ew_text::typeface_set` over the file the project
//! holds. A setting whose file the project does not hold, or whose bytes are no font, prints with the bundled set and
//! says so ([`TypefaceState`]). The query reads the asset store only when a typeface is set, and an image the project
//! gains gives the same value, so it redraws no page.

use std::collections::BTreeMap;
use std::fmt;

use ew_derive::{Policy, Q, Query};
use ew_dl::FontKey;
use ew_render_cpu::{FontBytes, FontResolver};
use ew_text::FontSet;

/// What became of the project's typeface setting.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TypefaceState {
    /// No typeface is set: the bundled faces print.
    Bundled,
    /// The file prints the Standard typeface.
    Loaded {
        /// The file's name, as the setting keeps it.
        name: String,
        /// The family its name table names (`Liberation Sans`).
        family: String,
        /// Its face, as the glyph runs name it ([`DrawingFonts`] has its file).
        face: FontKey,
    },
    /// The setting names a file the project does not hold: the bundled faces print.
    Missing {
        /// The file's name.
        name: String,
    },
    /// The file the setting names is no font the text stack reads: the bundled faces print.
    Unusable {
        /// The file's name.
        name: String,
    },
}

/// The fonts the project's texts are shaped with, and what became of its typeface.
pub struct ProjectFonts {
    /// The set: [`ew_text::bundled`] or the typeface's.
    pub set: &'static FontSet,
    /// The setting's outcome.
    pub state: TypefaceState,
}

impl PartialEq for ProjectFonts {
    fn eq(&self, other: &ProjectFonts) -> bool {
        std::ptr::eq(self.set, other.set) && self.state == other.state
    }
}

impl Eq for ProjectFonts {}

impl fmt::Debug for ProjectFonts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProjectFonts")
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

/// `project_fonts()`: the project's font set (see the module documentation).
#[derive(Debug)]
pub struct ProjectFontsQ;

impl Query for ProjectFontsQ {
    type Key = ();
    type Value = ProjectFonts;
    const FAMILY: &'static str = "project_fonts";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, (): &()) -> std::sync::Arc<ProjectFonts> {
        let bundled = || ProjectFonts {
            set: ew_text::bundled(),
            state: TypefaceState::Bundled,
        };
        let Some(t) = q.project().settings.typeface.clone() else {
            return std::sync::Arc::new(bundled());
        };
        let fonts = match q.typeface_bytes(&t.asset) {
            None => ProjectFonts {
                state: TypefaceState::Missing { name: t.name },
                ..bundled()
            },
            Some(bytes) => match ew_text::typeface_set(&bytes) {
                Ok(set) => {
                    let face = set.face_for(&ew_text::TextStyle::regular(ew_units::Nm(1)));
                    ProjectFonts {
                        set,
                        state: TypefaceState::Loaded {
                            name: t.name,
                            family: face.family_name.clone(),
                            face: face.key,
                        },
                    }
                }
                Err(_) => ProjectFonts {
                    state: TypefaceState::Unusable { name: t.name },
                    ..bundled()
                },
            },
        };
        std::sync::Arc::new(fonts)
    }
}

/// The font set the project's texts are shaped with.
pub fn fonts(q: &Q) -> &'static FontSet {
    q.get::<ProjectFontsQ>(()).set
}

impl crate::Session {
    /// What became of the project's typeface setting (the Text styles page's *Standard typeface* line).
    pub fn typeface(&self) -> TypefaceState {
        self.query().get::<ProjectFontsQ>(()).state.clone()
    }
}

/// The fonts the writers and renderers draw a display list's glyph runs with: the bundled faces
/// ([`super::font_bytes`]) and the face of every typeface this process built a set with (`ew_text::typeface_file`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct DrawingFonts;

impl FontResolver for DrawingFonts {
    fn font(&self, key: FontKey) -> Option<FontBytes> {
        super::font_bytes()
            .get(&key)
            .cloned()
            .or_else(|| ew_text::typeface_file(key).map(|data| FontBytes { data, index: 0 }))
    }
}

/// The files of `keys` as [`DrawingFonts`] finds them: what an adapter that renders a print set needs (the faces its
/// PDF embedded, `PdfReport::fonts`).
pub fn font_files(keys: impl IntoIterator<Item = FontKey>) -> BTreeMap<FontKey, FontBytes> {
    keys.into_iter()
        .filter_map(|k| Some((k, DrawingFonts.font(k)?)))
        .collect()
}
