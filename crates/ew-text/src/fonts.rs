// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The font set: bundled font files only, each identified by its SHA-256 ([`FontKey`]).

use std::fmt;
#[cfg(feature = "bundled-fonts")]
use std::sync::OnceLock;
use std::sync::{Arc, Mutex};

use ew_dl::FontKey;
use fontique::{Blob, Collection, CollectionOptions, FontInfoOverride, SourceCache};
use parley::{FontContext, LayoutContext};
use sha2::{Digest, Sha256};
use skrifa::MetadataProvider;
use skrifa::instance::{LocationRef, Size};
use skrifa::raw::TableProvider;

/// A font family role. Text styles name a role, not a family name, so the project's font set decides the
/// family (ADR-0028: Noto Sans for [`Family::Default`]).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub enum Family {
    /// The default drawing family.
    #[default]
    Default,
    /// A monospaced family (not bundled in v0; reserved for code-like texts).
    Mono,
}

/// Font weight. Only the two weights the shipped text styles use exist (ADR-0028 recommendation 1).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub enum Weight {
    /// 400.
    #[default]
    Regular,
    /// 700.
    Bold,
}

impl Weight {
    /// The CSS / OpenType weight value: 400 or 700.
    pub const fn value(self) -> u16 {
        match self {
            Weight::Regular => 400,
            Weight::Bold => 700,
        }
    }
}

/// Vertical metrics of a face in font units (y up in the font, given here as positive distances).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct FaceMetrics {
    /// Units per em.
    pub units_per_em: u16,
    /// Ascent above the baseline (positive).
    pub ascent: i32,
    /// Descent below the baseline (positive).
    pub descent: i32,
    /// Line gap.
    pub line_gap: i32,
    /// Cap height (`OS/2.sCapHeight`); 70 % of the em if the font has none.
    pub cap_height: i32,
}

impl FaceMetrics {
    /// The baseline-to-baseline distance of wrapped lines: ascent + descent + line gap.
    pub const fn line_pitch(&self) -> i32 {
        self.ascent + self.descent + self.line_gap
    }
}

/// One face of a [`FontSet`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FaceInfo {
    /// The file hash and face index.
    pub key: FontKey,
    /// The role it serves.
    pub family: Family,
    /// Its weight.
    pub weight: Weight,
    /// Whether it is the italic face.
    pub italic: bool,
    /// The family name from the font's name table (typographic family, else family).
    pub family_name: String,
    /// The version string from the font's name table (name ID 5).
    pub version: String,
    /// Its vertical metrics.
    pub metrics: FaceMetrics,
}

/// A font file offered to [`FontSet::new`], with the role, weight and slope it serves.
#[derive(Clone)]
pub struct FontSource {
    data: Blob<u8>,
    face_index: u16,
    family: Family,
    weight: Weight,
    italic: bool,
    expected: Option<[u8; 32]>,
}

impl fmt::Debug for FontSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FontSource")
            .field("len", &self.data.len())
            .field("face_index", &self.face_index)
            .field("family", &self.family)
            .field("weight", &self.weight)
            .field("italic", &self.italic)
            .finish()
    }
}

impl FontSource {
    /// A font file compiled into the binary.
    pub fn from_static(bytes: &'static [u8], family: Family, weight: Weight, italic: bool) -> FontSource {
        FontSource {
            data: Blob::new(Arc::new(bytes)),
            face_index: 0,
            family,
            weight,
            italic,
            expected: None,
        }
    }

    /// A font file read at run time (a project-embedded font).
    pub fn from_vec(bytes: Vec<u8>, family: Family, weight: Weight, italic: bool) -> FontSource {
        FontSource {
            data: Blob::from(bytes),
            face_index: 0,
            family,
            weight,
            italic,
            expected: None,
        }
    }

    /// This source with a pinned SHA-256: [`FontSet::new`] fails if the bytes hash differently
    /// (ADR-0028 recommendation 6).
    pub fn pinned(self, sha256: [u8; 32]) -> FontSource {
        FontSource {
            expected: Some(sha256),
            ..self
        }
    }

    /// This source using face `index` of a collection file.
    pub fn with_face_index(self, index: u16) -> FontSource {
        FontSource {
            face_index: index,
            ..self
        }
    }
}

/// Why a font set could not be built.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum FontError {
    /// A pinned file hashed differently.
    HashMismatch {
        /// The pinned hash.
        expected: [u8; 32],
        /// The hash of the bytes.
        actual: [u8; 32],
    },
    /// The bytes are not a font skrifa can read (or the face index does not exist).
    NotAFont {
        /// The file hash.
        sha256: [u8; 32],
    },
    /// Two sources serve the same role, weight and slope.
    Duplicate {
        /// Role.
        family: Family,
        /// Weight.
        weight: Weight,
        /// Slope.
        italic: bool,
    },
    /// No source was given.
    Empty,
}

impl fmt::Display for FontError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FontError::HashMismatch { expected, actual } => write!(
                f,
                "font file hash mismatch: expected {}, found {}",
                hex(expected),
                hex(actual)
            ),
            FontError::NotAFont { sha256 } => write!(f, "not a readable font: {}", hex(sha256)),
            FontError::Duplicate {
                family,
                weight,
                italic,
            } => write!(f, "two fonts for {family:?} {weight:?} italic={italic}"),
            FontError::Empty => write!(f, "no font given"),
        }
    }
}

impl std::error::Error for FontError {}

/// Lower-case hex of a hash.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub(crate) struct Face {
    pub(crate) info: FaceInfo,
    pub(crate) data: Blob<u8>,
}

impl Face {
    /// The face as a skrifa font reference; the bytes were validated when the set was built.
    pub(crate) fn font_ref(&self) -> skrifa::FontRef<'_> {
        skrifa::FontRef::from_index(self.data.data(), u32::from(self.info.key.face_index))
            .expect("validated when the font set was built")
    }
}

/// Parley's font database and layout scratch space. `shape` locks it for the duration of one layout.
pub(crate) struct Engine {
    pub(crate) fonts: FontContext,
    pub(crate) layout: LayoutContext<()>,
}

/// The fonts text is shaped with: bundled or project-embedded files only, never system fonts (NFR-REL-11,
/// ADR-0028).
///
/// Faces are kept sorted by (role, weight, slope, key), so the order in which files are offered never
/// changes the result. Parley's `fontique` collection is built with system fonts disabled and without
/// fallback families: a character the selected face lacks shapes to its `.notdef` glyph and is reported
/// in [`Shaped::missing`](crate::Shaped::missing) (FR-OUT-027).
pub struct FontSet {
    pub(crate) faces: Vec<Face>,
    pub(crate) engine: Mutex<Engine>,
}

impl fmt::Debug for FontSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FontSet")
            .field("faces", &self.faces.iter().map(|x| &x.info).collect::<Vec<_>>())
            .finish()
    }
}

impl FontSet {
    /// A font set from `sources`.
    ///
    /// # Errors
    ///
    /// [`FontError`] if a pinned hash does not match, a file is unreadable, two files serve the same
    /// role, weight and slope, or `sources` is empty.
    pub fn new(sources: Vec<FontSource>) -> Result<FontSet, FontError> {
        if sources.is_empty() {
            return Err(FontError::Empty);
        }
        let mut faces = Vec::with_capacity(sources.len());
        for src in sources {
            let sha256: [u8; 32] = Sha256::digest(src.data.data()).into();
            if let Some(expected) = src.expected
                && expected != sha256
            {
                return Err(FontError::HashMismatch {
                    expected,
                    actual: sha256,
                });
            }
            let font = skrifa::FontRef::from_index(src.data.data(), u32::from(src.face_index))
                .map_err(|_| FontError::NotAFont { sha256 })?;
            let info = FaceInfo {
                key: FontKey {
                    sha256,
                    face_index: src.face_index,
                },
                family: src.family,
                weight: src.weight,
                italic: src.italic,
                family_name: family_name(&font).ok_or(FontError::NotAFont { sha256 })?,
                version: name_string(&font, skrifa::raw::types::NameId::VERSION_STRING).unwrap_or_default(),
                metrics: metrics(&font).ok_or(FontError::NotAFont { sha256 })?,
            };
            faces.push(Face { info, data: src.data });
        }
        faces.sort_by(|a, b| {
            let k = |f: &Face| (f.info.family, f.info.weight, f.info.italic, f.info.key);
            k(a).cmp(&k(b))
        });
        if let Some(w) = faces.windows(2).find(|w| {
            let k = |f: &Face| (f.info.family, f.info.weight, f.info.italic);
            k(&w[0]) == k(&w[1])
        }) {
            return Err(FontError::Duplicate {
                family: w[0].info.family,
                weight: w[0].info.weight,
                italic: w[0].info.italic,
            });
        }
        let mut collection = Collection::new(CollectionOptions {
            shared: false,
            system_fonts: false,
        });
        for face in &faces {
            // Each face is registered under a private family name per role, so two roles that share a
            // family name in their name tables stay apart, and the weight and slope parley matches on are
            // the ones this set declares.
            let name = engine_family(face.info.family);
            collection.register_fonts(
                face.data.clone(),
                Some(FontInfoOverride {
                    family_name: Some(name),
                    weight: Some(parley::FontWeight::new(f32::from(face.info.weight.value()))),
                    style: Some(if face.info.italic {
                        parley::FontStyle::Italic
                    } else {
                        parley::FontStyle::Normal
                    }),
                    ..FontInfoOverride::default()
                }),
            );
        }
        Ok(FontSet {
            faces,
            engine: Mutex::new(Engine {
                fonts: FontContext {
                    collection,
                    source_cache: SourceCache::default(),
                },
                layout: LayoutContext::new(),
            }),
        })
    }

    /// The faces, in their canonical order.
    pub fn faces(&self) -> impl ExactSizeIterator<Item = &FaceInfo> + '_ {
        self.faces.iter().map(|f| &f.info)
    }

    /// The bytes of the whole font file behind `key` (the face is `key.face_index` inside it), for the
    /// renderers and writers that draw or embed the glyphs this set shaped (WP-S3.04); `None` if the set
    /// has no such face.
    pub fn face_data(&self, key: FontKey) -> Option<&[u8]> {
        self.faces
            .iter()
            .find(|f| f.info.key == key)
            .map(|f| f.data.data())
    }

    /// The face a style selects: the exact (role, weight, slope) match; else the role's upright face of
    /// that weight; else the role's upright regular face; else the first face of the set.
    pub(crate) fn select(&self, family: Family, weight: Weight, italic: bool) -> &Face {
        let find = |w: Weight, i: bool| {
            self.faces
                .iter()
                .find(|f| f.info.family == family && f.info.weight == w && f.info.italic == i)
        };
        find(weight, italic)
            .or_else(|| find(weight, false))
            .or_else(|| find(Weight::Regular, false))
            .unwrap_or(&self.faces[0])
    }

    /// The face `style` selects: the exact (role, weight, slope) match; else the role's upright face of
    /// that weight; else the role's upright regular face; else the first face of the set.
    pub fn face_for(&self, style: &crate::TextStyle) -> &FaceInfo {
        &self.select(style.family, style.weight, style.italic).info
    }

    /// Whether the face selected by `style` has a glyph for `ch`.
    pub fn covers(&self, style: &crate::TextStyle, ch: char) -> bool {
        self.select(style.family, style.weight, style.italic)
            .font_ref()
            .charmap()
            .map(ch)
            .is_some()
    }
}

/// The private family name a role is registered under in the parley collection.
pub(crate) const fn engine_family(family: Family) -> &'static str {
    match family {
        Family::Default => "ew-default",
        Family::Mono => "ew-mono",
    }
}

fn name_string(font: &skrifa::FontRef<'_>, id: skrifa::raw::types::NameId) -> Option<String> {
    font.localized_strings(id)
        .english_or_first()
        .map(|s| s.chars().collect())
}

fn family_name(font: &skrifa::FontRef<'_>) -> Option<String> {
    name_string(font, skrifa::raw::types::NameId::TYPOGRAPHIC_FAMILY_NAME)
        .or_else(|| name_string(font, skrifa::raw::types::NameId::FAMILY_NAME))
}

/// The unscaled metrics of a face. skrifa reports font units as `f32` at [`Size::unscaled`]; for the
/// integer fields of `hhea`/`OS/2` these are exact, and they are rounded defensively.
fn metrics(font: &skrifa::FontRef<'_>) -> Option<FaceMetrics> {
    let m = font.metrics(Size::unscaled(), LocationRef::default());
    let upem = m.units_per_em;
    if upem == 0 {
        return None;
    }
    let cap = font
        .os2()
        .ok()
        .and_then(|os2| os2.s_cap_height())
        .map(i32::from)
        .filter(|&c| c > 0)
        .unwrap_or(i32::from(upem) * 7 / 10);
    Some(FaceMetrics {
        units_per_em: upem,
        ascent: m.ascent.round() as i32,
        descent: (-m.descent).round() as i32,
        line_gap: m.leading.round() as i32,
        cap_height: cap,
    })
}

/// The SHA-256 of the bundled Noto Sans Regular file (`assets/fonts/NotoSans-Regular.ttf`; provenance in
/// `NOTICE`). Pinned: a different file is an error (ADR-0028 recommendation 6, NFR-REL-17).
pub const NOTO_SANS_REGULAR_SHA256: [u8; 32] = [
    0xd8, 0x9e, 0x04, 0x48, 0x85, 0x4c, 0xb0, 0xf1, 0x1e, 0x06, 0x09, 0xed, 0x47, 0x25, 0x11, 0xe5, 0x9b,
    0x12, 0x96, 0x6a, 0x4c, 0x8e, 0xf3, 0x7a, 0x37, 0x06, 0x75, 0x2a, 0x78, 0xbe, 0x0a, 0xda,
];
/// The SHA-256 of the bundled Noto Sans Bold file (`assets/fonts/NotoSans-Bold.ttf`).
pub const NOTO_SANS_BOLD_SHA256: [u8; 32] = [
    0x8c, 0xaa, 0xc5, 0x7b, 0x9e, 0xd4, 0x92, 0xab, 0x8a, 0x05, 0x01, 0xf1, 0xf1, 0x57, 0x3d, 0x83, 0xa0,
    0x88, 0x22, 0xbe, 0x36, 0xdd, 0x3f, 0x25, 0x5e, 0x05, 0x96, 0x4e, 0x12, 0x10, 0x5f, 0x8d,
];

#[cfg(feature = "bundled-fonts")]
const NOTO_SANS_REGULAR: &[u8] = include_bytes!("../../../assets/fonts/NotoSans-Regular.ttf");
#[cfg(feature = "bundled-fonts")]
const NOTO_SANS_BOLD: &[u8] = include_bytes!("../../../assets/fonts/NotoSans-Bold.ttf");

/// The bundled sources: Noto Sans Regular and Bold as [`Family::Default`], pinned by hash.
#[cfg(feature = "bundled-fonts")]
pub fn bundled_sources() -> Vec<FontSource> {
    vec![
        FontSource::from_static(NOTO_SANS_REGULAR, Family::Default, Weight::Regular, false)
            .pinned(NOTO_SANS_REGULAR_SHA256),
        FontSource::from_static(NOTO_SANS_BOLD, Family::Default, Weight::Bold, false)
            .pinned(NOTO_SANS_BOLD_SHA256),
    ]
}

/// The process-wide bundled font set, built on first use.
///
/// # Panics
///
/// If a bundled file does not match its pinned hash: the build embedded the wrong font (ADR-0028
/// recommendation 6).
#[cfg(feature = "bundled-fonts")]
pub fn bundled() -> &'static FontSet {
    static SET: OnceLock<FontSet> = OnceLock::new();
    SET.get_or_init(|| FontSet::new(bundled_sources()).expect("bundled fonts match their pinned hashes"))
}
