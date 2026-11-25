// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Per-project text styles (ADR-0032, FR-SYM-013): the member `ProjectSettings.text_styles`, the built-in
//! table it overrides and the effective table every renderer and EW-CORE-061 read.
//!
//! A project stores its text styles as **overrides** of the built-in table
//! ([`BUILTIN_TEXT_STYLES`]): a JSON object keyed by style key, each value `{"size": <em height in nm>,
//! "weight": "regular" | "bold", "face": "standard" | "narrow" | "mono" | "technical_drawing"}` (weight and face optional; the
//! face since 0.3.0, ADR-0028 amendments 2 and 3) plus unknown members kept verbatim. An absent or empty
//! member means no overrides, so every file written before ADR-0032 renders as before; nothing writes the
//! member into a file that lacks it. A key in both takes the override's size and, when given, its weight; a key
//! only in the overrides extends the table ([`effective_text_styles`]).
//!
//! ```
//! use ew_schema::{ProjectSettings, effective_text_styles, new_project_text_styles};
//! use ew_units::Nm;
//!
//! let plain = effective_text_styles(&ProjectSettings::default());
//! let marker = plain.iter().find(|s| s.key.as_str() == "marker").unwrap();
//! assert_eq!(marker.em, Nm(1_322_917)); // 5 QET px
//!
//! let settings = ProjectSettings { text_styles: new_project_text_styles(), ..ProjectSettings::default() };
//! let fresh = effective_text_styles(&settings);
//! let marker = fresh.iter().find(|s| s.key.as_str() == "marker").unwrap();
//! assert_eq!(marker.em, Nm(1_852_083)); // 7 QET px, 1.32 mm cap height
//! assert_eq!(fresh.len(), plain.len());
//! ```

use std::collections::BTreeMap;

use ew_mltext::Ident;
use ew_units::{Grid, Nm};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::json::{Extra, de_extra};
use crate::project::ProjectSettings;
use crate::proxy::NmS;
use crate::track::open_enum;

/// The weight of a text style (ADR-0032 decision 1).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "TextWeight")]
pub enum TextWeight {
    /// Regular.
    Regular,
    /// Bold.
    Bold,
    /// A weight of a newer format, kept verbatim; the effective table reads it as absent.
    #[serde(skip)]
    Unknown(Value),
}

/// The bundled typeface of a text style (ADR-0028 amendment 2): every face is compiled into the program, so a drawing
/// looks the same on every machine; system fonts are never used.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "Typeface")]
pub enum Typeface {
    /// Noto Sans, the default drawing face.
    #[default]
    Standard,
    /// Noto Sans Condensed: narrower letters for long texts in tight places.
    Narrow,
    /// Noto Sans Mono: every character as wide as the next, for columns of numbers and addresses.
    Mono,
    /// Routed Gothic: the single-stroke lettering of technical drawings and engraved signs. It has no bold: a bold
    /// style in it prints regular ([`Typeface::has_bold`]).
    TechnicalDrawing,
    /// A face of a newer format, kept verbatim; the effective table reads it as [`Typeface::Standard`].
    #[serde(skip)]
    Unknown(Value),
}

/// The faces a person chooses from, in the order the Text styles page lists them.
pub const TYPEFACES: [Typeface; 4] = [
    Typeface::Standard,
    Typeface::Narrow,
    Typeface::Mono,
    Typeface::TechnicalDrawing,
];

impl Typeface {
    /// The member's word (`standard`, `narrow`, `mono`, `technical_drawing`); `None` for a face of a newer format.
    pub fn word(&self) -> Option<&'static str> {
        match self {
            Typeface::Standard => Some("standard"),
            Typeface::Narrow => Some("narrow"),
            Typeface::Mono => Some("mono"),
            Typeface::TechnicalDrawing => Some("technical_drawing"),
            Typeface::Unknown(_) => None,
        }
    }

    /// Whether the face has a bold cut. Without one a bold style prints regular; the style keeps its weight, so it
    /// is bold again in a face that has one.
    pub fn has_bold(&self) -> bool {
        !matches!(self, Typeface::TechnicalDrawing)
    }

    /// The face of `word` ([`Typeface::word`]).
    pub fn from_word(word: &str) -> Option<Typeface> {
        TYPEFACES.into_iter().find(|t| t.word() == Some(word))
    }
}

/// One entry of `ProjectSettings.text_styles`: an override of a built-in style or a new style (ADR-0032).
///
/// Italic and alignment are not typed (no italic face is bundled, ADR-0028; alignment belongs to the slot) and stay in
/// `extra`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct TextStyleSpec {
    /// The em height in nm, 500,000 … 50,000,000 (0.5 mm … 50 mm).
    #[schemars(with = "NmS")]
    pub size: Nm,
    /// The weight; absent keeps the built-in row's weight (regular for a new style).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<TextWeight>,
    /// The typeface; absent is [`Typeface::Standard`], so a file without it prints as before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub face: Option<Typeface>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

impl TextStyleSpec {
    /// An entry of `size` without a weight.
    pub fn of_size(size: Nm) -> TextStyleSpec {
        TextStyleSpec {
            size,
            weight: None,
            face: None,
            extra: Extra::new(),
        }
    }
}

/// The smallest em height a text style may have: 0.5 mm (ADR-0032 decision 1).
pub const TEXT_STYLE_MIN_SIZE: Nm = Nm(500_000);

/// The largest em height a text style may have: 50 mm (ADR-0032 decision 1).
pub const TEXT_STYLE_MAX_SIZE: Nm = Nm(50_000_000);

/// What a text style prints, as the Text styles page groups the styles: by what a person sees on a drawing.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum TextStyleGroup {
    /// Device tags, their details, connection points and cross-references.
    Devices,
    /// Wire marks and the cores of a cable.
    Wires,
    /// Terminal numbers.
    Terminals,
    /// The page's own texts: free texts, tables and boxes.
    Page,
    /// The cover and the section pages.
    Cover,
}

/// The built-in text-style table as `(key, size in QET px, weight, group)`, in table order: the base
/// every project's overrides apply to (ADR-0032 decision 4: part of the derivation profile; a change of a row is a
/// derivation-profile change). `ew_text::builtin()` is built from these rows for callers without a project.
/// `terminal`, the terminal's number, came out of `tag.main` and keeps its size, so a
/// project that sets neither prints as before.
pub const BUILTIN_TEXT_STYLES: &[(&str, u16, TextWeight, TextStyleGroup)] = &[
    ("cover.title", 48, TextWeight::Regular, TextStyleGroup::Cover),
    ("section.id", 37, TextWeight::Bold, TextStyleGroup::Cover),
    ("section.subtitle", 29, TextWeight::Regular, TextStyleGroup::Cover),
    ("cover.subtitle", 27, TextWeight::Regular, TextStyleGroup::Cover),
    ("box.title", 21, TextWeight::Regular, TextStyleGroup::Page),
    ("cover.table", 16, TextWeight::Regular, TextStyleGroup::Page),
    ("tag.main", 13, TextWeight::Regular, TextStyleGroup::Devices),
    ("text.normal", 12, TextWeight::Regular, TextStyleGroup::Page),
    ("field.label", 11, TextWeight::Regular, TextStyleGroup::Page),
    ("data.small", 9, TextWeight::Regular, TextStyleGroup::Devices),
    ("pin", 8, TextWeight::Regular, TextStyleGroup::Devices),
    ("xref.device", 7, TextWeight::Regular, TextStyleGroup::Devices),
    ("cp.small", 5, TextWeight::Regular, TextStyleGroup::Devices),
    ("marker", 5, TextWeight::Regular, TextStyleGroup::Wires),
    ("terminal", 13, TextWeight::Regular, TextStyleGroup::Terminals),
];

/// Whether `symbol`, drawn for the function definition `def`, draws one terminal ([`slot_default_style`]): the
/// definition's category carries the `terminal` flag of the category table (`ew_rds::category_flags`), and the symbol
/// is no strip segment (which draws a strip, its tag the strip's).
pub fn draws_terminal(symbol: &crate::SymbolDef, def: &crate::FnDef) -> bool {
    ew_rds::category_flags(def.category.0.as_str()).terminal
        && symbol.rep_type != crate::RepType::StripSegment
}

/// The text style a slot that names none prints with, one place for the page scene and EW-CORE-061: the terminal's
/// number (`terminal`) for the tag of a symbol that draws a terminal (`terminal`, [`draws_terminal`]), `tag.main` for
/// any other tag, `cp.small` for a CP designation, `text.normal` for a kind of a newer format and `data.small` for
/// every other kind.
pub fn slot_default_style(terminal: bool, kind: &crate::SlotKind) -> &'static str {
    use crate::SlotKind;
    match kind {
        SlotKind::Tag if terminal => "terminal",
        SlotKind::Tag => "tag.main",
        SlotKind::CpDesignation(_) => "cp.small",
        SlotKind::Unknown(_) => "text.normal",
        _ => "data.small",
    }
}

/// The group of style `key`: its built-in row's; a style a project adds is the page's own.
pub fn text_style_group(key: &str) -> TextStyleGroup {
    BUILTIN_TEXT_STYLES
        .iter()
        .find(|(k, ..)| *k == key)
        .map_or(TextStyleGroup::Page, |(.., g)| *g)
}

/// The cap ratio of every bundled face, in thousandths: a capital letter is 0.714 of the em height (the
/// `OS/2` cap height of Noto Sans, Noto Sans Condensed and Noto Sans Mono, 714 of 1000 units, which a test in `ew-text`
/// keeps). Routed Gothic's is 736 of 1024 units (0.719): its capitals print at most 1 % higher, never lower, so a style
/// that reads as readable here is.
pub const CAP_RATIO_MILLI: i64 = 714;

/// The smallest printed cap height that is readable on paper: 1.2 mm (EW-CORE-061 reports a style
/// below it, and the Text styles page notes it at the row).
pub const MIN_CAP_HEIGHT: Nm = Nm(1_200_000);

/// The printed cap height of an em height: `em × 0.714`, the height of a capital as a person measures it on paper and
/// as ISO 3098 names a text's size.
pub fn cap_height(em: Nm) -> Nm {
    Nm(em.0 * CAP_RATIO_MILLI / 1000)
}

/// The smallest em height whose [`cap_height`] is at least `cap`.
///
/// ```
/// use ew_units::Nm;
///
/// let em = ew_schema::em_for_cap(Nm(1_200_000));
/// assert_eq!(ew_schema::cap_height(em), Nm(1_200_000));
/// assert!(ew_schema::cap_height(Nm(em.0 - 1)) < Nm(1_200_000));
/// ```
pub fn em_for_cap(cap: Nm) -> Nm {
    Nm((cap.0 * 1000 + CAP_RATIO_MILLI - 1) / CAP_RATIO_MILLI)
}

/// The overrides a new project starts with (ADR-0032 decision 3): `cp.small` and `marker` at `Grid::QET_1PX.at(7)` =
/// 1,852,083 nm em height, a printed cap height of 1.32 mm, and `terminal` at a cap height of 1.8 mm, a standard height
/// below the device tag's. Every style of a new project prints at least [`MIN_CAP_HEIGHT`] high.
pub const NEW_PROJECT_TEXT_STYLES: &[(&str, Nm)] = &[
    ("cp.small", Nm(1_852_083)),
    ("marker", Nm(1_852_083)),
    ("terminal", Nm(2_521_009)),
];

/// [`NEW_PROJECT_TEXT_STYLES`] as the member's value.
pub fn new_project_text_styles() -> BTreeMap<Ident, TextStyleSpec> {
    NEW_PROJECT_TEXT_STYLES
        .iter()
        .filter_map(|(key, size)| Some((key.parse().ok()?, TextStyleSpec::of_size(*size))))
        .collect()
}

/// The em height of `px` QET pixels: `round(px × 793 750 / 3)` nm.
pub fn qet_px_em(px: u16) -> Nm {
    Grid::QET_1PX.at(i64::from(px)).unwrap_or(Nm::ZERO)
}

/// One row of the effective text-style table (ADR-0032 decision 1).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EffectiveStyle {
    /// The style key.
    pub key: Ident,
    /// The em height.
    pub em: Nm,
    /// The weight: [`TextWeight::Regular`] or [`TextWeight::Bold`], never `Unknown`.
    pub weight: TextWeight,
    /// The typeface, never `Unknown`.
    pub face: Typeface,
}

impl EffectiveStyle {
    /// Whether the weight is bold.
    pub fn is_bold(&self) -> bool {
        self.weight == TextWeight::Bold
    }
}

/// Whether `size` lies in [`TEXT_STYLE_MIN_SIZE`] … [`TEXT_STYLE_MAX_SIZE`].
pub fn text_style_size_ok(size: Nm) -> bool {
    (TEXT_STYLE_MIN_SIZE..=TEXT_STYLE_MAX_SIZE).contains(&size)
}

/// The built-in styles that print as another style of the project when the project sets that one and not them, as
/// `(style, the style it follows)`: `terminal` came out of `tag.main` (ADR-0032 amendment 1), so a project made
/// before it that sets the device tag's size prints its terminal numbers at that size as it did.
const FOLLOWS: &[(&str, &str)] = &[("terminal", "tag.main")];

/// The effective text-style table of `settings` (ADR-0032 decision 1): the rows of [`BUILTIN_TEXT_STYLES`] in their
/// order, each with its override applied, then the extending keys in key order. A style of [`FOLLOWS`] without an
/// override of its own takes the style it follows when that one has an override (`terminal` prints as `tag.main`).
///
/// An override whose size lies outside [`TEXT_STYLE_MIN_SIZE`] … [`TEXT_STYLE_MAX_SIZE`] (a hand-edited file;
/// `project.settings.set` refuses it) is ignored: the built-in row stays, and an extending key is not added. A
/// weight or a typeface of a newer format reads as absent.
pub fn effective_text_styles(settings: &ProjectSettings) -> Vec<EffectiveStyle> {
    let overrides = &settings.text_styles;
    let weight = |spec: &TextStyleSpec, base: &TextWeight| match &spec.weight {
        Some(w @ (TextWeight::Bold | TextWeight::Regular)) => w.clone(),
        _ => base.clone(),
    };
    let face = |spec: &TextStyleSpec| match &spec.face {
        Some(Typeface::Unknown(_)) | None => Typeface::Standard,
        Some(f) => f.clone(),
    };
    let mut rows: Vec<EffectiveStyle> = BUILTIN_TEXT_STYLES
        .iter()
        .filter_map(|(key, px, w, _)| {
            let key: Ident = key.parse().ok()?;
            Some(
                match overrides.get(key.as_str()).filter(|s| text_style_size_ok(s.size)) {
                    Some(spec) => EffectiveStyle {
                        em: spec.size,
                        weight: weight(spec, w),
                        face: face(spec),
                        key,
                    },
                    None => EffectiveStyle {
                        em: qet_px_em(*px),
                        weight: w.clone(),
                        face: Typeface::Standard,
                        key,
                    },
                },
            )
        })
        .collect();
    let set = |key: &str| overrides.get(key).is_some_and(|s| text_style_size_ok(s.size));
    for (style, follows) in FOLLOWS
        .iter()
        .filter(|(style, follows)| !set(style) && set(follows))
    {
        let parent = rows.iter().find(|r| r.key.as_str() == *follows).cloned();
        if let (Some(parent), Some(row)) = (parent, rows.iter_mut().find(|r| r.key.as_str() == *style)) {
            row.em = parent.em;
            row.weight = parent.weight;
            row.face = parent.face;
        }
    }
    for (key, spec) in overrides {
        if text_style_size_ok(spec.size) && !BUILTIN_TEXT_STYLES.iter().any(|(k, ..)| *k == key.as_str()) {
            rows.push(EffectiveStyle {
                key: key.clone(),
                em: spec.size,
                weight: weight(spec, &TextWeight::Regular),
                face: face(spec),
            });
        }
    }
    rows
}

/// The built-in row of `key` as the style a project prints with when it sets nothing: its em height, weight and the
/// standard face; `None` for a key the built-in table lacks.
pub fn builtin_text_style(key: &str) -> Option<EffectiveStyle> {
    let (k, px, weight, _) = BUILTIN_TEXT_STYLES.iter().find(|(k, ..)| *k == key)?;
    Some(EffectiveStyle {
        key: k.parse().ok()?,
        em: qet_px_em(*px),
        weight: weight.clone(),
        face: Typeface::Standard,
    })
}

/// The style `key` has in a new project ([`NEW_PROJECT_TEXT_STYLES`] over the built-in row): what Reset and Standard
/// sizes on the Text styles page return it to; `None` for a key the built-in table lacks.
pub fn standard_text_style(key: &str) -> Option<EffectiveStyle> {
    let mut row = builtin_text_style(key)?;
    if let Some((_, em)) = NEW_PROJECT_TEXT_STYLES.iter().find(|(k, _)| *k == key) {
        row.em = *em;
    }
    Some(row)
}

/// The entry of `ProjectSettings.text_styles` that makes `key` print as `style`: `None` when the built-in row already
/// does and `kept` (the entry before) holds no unknown members, else the size and, where they differ from the
/// built-in row, the weight and the typeface, with the unknown members of `kept`. A style of [`FOLLOWS`] always gets
/// its entry, so that it keeps what it was given when the style it follows has an override.
pub fn text_style_entry(
    key: &str,
    style: &EffectiveStyle,
    kept: Option<&TextStyleSpec>,
) -> Option<TextStyleSpec> {
    let base = builtin_text_style(key);
    let extra = kept.map(|k| k.extra.clone()).unwrap_or_default();
    let follows = FOLLOWS.iter().any(|(k, _)| *k == key);
    if extra.is_empty() && !follows && base.as_ref() == Some(style) {
        return None;
    }
    let base_weight = base.as_ref().map_or(TextWeight::Regular, |b| b.weight.clone());
    Some(TextStyleSpec {
        size: style.em,
        weight: (style.weight != base_weight).then(|| style.weight.clone()),
        face: (style.face != Typeface::Standard).then(|| style.face.clone()),
        extra,
    })
}

/// Why a value for `text_styles` is refused ([`check_text_styles`]).
#[derive(Debug)]
pub struct TextStyleError {
    /// The JSON Pointer of the refused value below the member (`/tag.main/size`; empty for the whole value).
    pub pointer: String,
    /// What is wrong: the facts, which the application words.
    pub problem: TextStyleProblem,
}

/// What is wrong with a value for `text_styles` ([`TextStyleError`]).
#[derive(Debug)]
pub enum TextStyleProblem {
    /// The value is not an object.
    NotAnObject,
    /// A style key is not an identifier.
    Key {
        /// The key as given.
        key: String,
        /// Why it is not an identifier.
        error: ew_mltext::TextError,
    },
    /// A style is not an object.
    StyleNotAnObject {
        /// The style key.
        key: String,
    },
    /// A style has no integer size.
    NoSize {
        /// The style key.
        key: String,
    },
    /// A style's size lies outside [`TEXT_STYLE_MIN_SIZE`] … [`TEXT_STYLE_MAX_SIZE`].
    Size {
        /// The style key.
        key: String,
        /// The size given.
        size: Nm,
    },
    /// A style's weight is not `regular` or `bold`.
    Weight {
        /// The style key.
        key: String,
        /// The weight given.
        weight: Value,
    },
    /// A style's typeface is none of [`TYPEFACES`].
    Face {
        /// The style key.
        key: String,
        /// The typeface given.
        face: Value,
    },
    /// The style does not read as a [`TextStyleSpec`].
    Entry(serde_json::Error),
}

/// A JSON Pointer reference token (RFC 6901: `~` as `~0`, `/` as `~1`).
fn token(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

/// Checks a whole value for `text_styles` as an editing command receives it (ADR-0032 decision 6) and reads it:
/// an object whose keys are identifiers and whose entries have an integer `size` in [`TEXT_STYLE_MIN_SIZE`] …
/// [`TEXT_STYLE_MAX_SIZE`] and, when present, a `weight` of `regular` or `bold`. Unknown members of an entry are
/// kept.
///
/// # Errors
///
/// The first refused value in key order, with its pointer below the member.
///
/// ```
/// use serde_json::json;
///
/// assert!(ew_schema::check_text_styles(&json!({"tag.main": {"size": 4_000_000}})).is_ok());
/// let e = ew_schema::check_text_styles(&json!({"tag.main": {"size": 100}})).unwrap_err();
/// assert_eq!(e.pointer, "/tag.main/size");
/// ```
pub fn check_text_styles(value: &Value) -> Result<BTreeMap<Ident, TextStyleSpec>, TextStyleError> {
    let err = |pointer: String, problem: TextStyleProblem| TextStyleError { pointer, problem };
    let Value::Object(map) = value else {
        return Err(err(String::new(), TextStyleProblem::NotAnObject));
    };
    let mut out = BTreeMap::new();
    for (key, entry) in map {
        let at = format!("/{}", token(key));
        let ident: Ident = key.parse().map_err(|error| {
            err(
                at.clone(),
                TextStyleProblem::Key {
                    key: key.clone(),
                    error,
                },
            )
        })?;
        let Value::Object(members) = entry else {
            return Err(err(at, TextStyleProblem::StyleNotAnObject { key: key.clone() }));
        };
        let size = members.get("size").and_then(Value::as_i64);
        let Some(size) = size.map(Nm) else {
            return Err(err(
                format!("{at}/size"),
                TextStyleProblem::NoSize { key: key.clone() },
            ));
        };
        if !text_style_size_ok(size) {
            return Err(err(
                format!("{at}/size"),
                TextStyleProblem::Size {
                    key: key.clone(),
                    size,
                },
            ));
        }
        // No let chain: the permissive crates build on Rust 1.85 (the MSRV their manifests declare).
        if let Some(w) = members
            .get("weight")
            .filter(|w| !matches!(w.as_str(), Some("regular" | "bold")))
        {
            return Err(err(
                format!("{at}/weight"),
                TextStyleProblem::Weight {
                    key: key.clone(),
                    weight: w.clone(),
                },
            ));
        }
        if let Some(f) = members
            .get("face")
            .filter(|f| !f.as_str().is_some_and(|w| Typeface::from_word(w).is_some()))
        {
            return Err(err(
                format!("{at}/face"),
                TextStyleProblem::Face {
                    key: key.clone(),
                    face: f.clone(),
                },
            ));
        }
        let spec: TextStyleSpec =
            serde_json::from_value(entry.clone()).map_err(|e| err(at.clone(), TextStyleProblem::Entry(e)))?;
        out.insert(ident, spec);
    }
    Ok(out)
}

open_enum!(TextWeight, Typeface);

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn builtin_rows_are_unique_and_a_new_project_prints_every_style_readably() {
        let mut keys: Vec<&str> = BUILTIN_TEXT_STYLES.iter().map(|(k, ..)| *k).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), BUILTIN_TEXT_STYLES.len());
        for (key, _) in NEW_PROJECT_TEXT_STYLES {
            assert!(BUILTIN_TEXT_STYLES.iter().any(|(k, ..)| k == key), "{key}");
        }
        assert_eq!(new_project_text_styles().len(), NEW_PROJECT_TEXT_STYLES.len());
        let settings = ProjectSettings {
            text_styles: new_project_text_styles(),
            ..ProjectSettings::default()
        };
        for row in effective_text_styles(&settings) {
            assert!(cap_height(row.em) >= MIN_CAP_HEIGHT, "{}", row.key);
        }
        // The terminal's number is smaller than the device tag in a new project, and as large where nothing is set.
        let em = |s: &ProjectSettings, k: &str| {
            effective_text_styles(s)
                .into_iter()
                .find(|r| r.key.as_str() == k)
                .unwrap()
                .em
        };
        assert!(em(&settings, "terminal") < em(&settings, "tag.main"));
        let plain = ProjectSettings::default();
        assert_eq!(em(&plain, "terminal"), em(&plain, "tag.main"));
        assert_eq!(cap_height(em(&settings, "terminal")), Nm(1_800_000));
    }

    /// A project made before the `terminal` style that sets the device tag's size prints its terminal
    /// numbers at that size, its weight and face; a terminal style of its own (a new project's) wins.
    #[test]
    fn the_terminal_style_follows_the_device_tag_of_a_project_that_sets_only_it() {
        let settings = |v: Value| ProjectSettings {
            text_styles: check_text_styles(&v).unwrap(),
            ..ProjectSettings::default()
        };
        let row = |s: &ProjectSettings, k: &str| {
            effective_text_styles(s)
                .into_iter()
                .find(|r| r.key.as_str() == k)
                .unwrap()
        };
        let old = settings(json!({"tag.main": {"size": 4_000_000, "weight": "bold", "face": "narrow"}}));
        let terminal = row(&old, "terminal");
        assert_eq!(
            (terminal.em, terminal.weight, terminal.face),
            (Nm(4_000_000), TextWeight::Bold, Typeface::Narrow)
        );
        let own = settings(json!({"tag.main": {"size": 4_000_000}, "terminal": {"size": 2_521_009}}));
        assert_eq!(row(&own, "terminal").em, Nm(2_521_009));
        assert_eq!(row(&ProjectSettings::default(), "terminal").em, qet_px_em(13));
        // Set back to its built-in row, the terminal style keeps an entry, so it does not follow the device tag again.
        let base = builtin_text_style("terminal").unwrap();
        assert_eq!(
            text_style_entry("terminal", &base, None).map(|e| e.size),
            Some(qet_px_em(13))
        );
    }

    #[test]
    fn an_entry_holds_only_what_differs_from_the_built_in_row() {
        let base = builtin_text_style("marker").unwrap();
        assert_eq!(text_style_entry("marker", &base, None), None);
        let mut bold = base.clone();
        bold.weight = TextWeight::Bold;
        bold.face = Typeface::Narrow;
        let entry = text_style_entry("marker", &bold, None).unwrap();
        assert_eq!(
            serde_json::to_value(&entry).unwrap(),
            json!({"size": 1_322_917, "weight": "bold", "face": "narrow"})
        );
        // Unknown members of the entry before stay, also when the style returns to its built-in row.
        let kept = check_text_styles(&json!({"marker": {"size": 1, "x": 1}}));
        assert!(kept.is_err());
        let kept: TextStyleSpec = serde_json::from_value(json!({"size": 2_000_000, "x": 1})).unwrap();
        let entry = text_style_entry("marker", &base, Some(&kept)).unwrap();
        assert_eq!(entry.extra["x"], json!(1));
        assert_eq!(standard_text_style("marker").unwrap().em, Nm(1_852_083));
    }

    #[test]
    fn overrides_replace_and_extend() {
        let mut settings = ProjectSettings::default();
        let base = effective_text_styles(&settings);
        assert_eq!(base.len(), BUILTIN_TEXT_STYLES.len());
        settings.text_styles = check_text_styles(&json!({
            "tag.main": {"size": 4_000_000, "weight": "bold"},
            "note.big": {"size": 6_000_000},
            "a.first": {"size": 1_000_000, "weight": "bold"},
            "section.id": {"size": 9_000_000}
        }))
        .unwrap();
        let rows = effective_text_styles(&settings);
        let keys: Vec<&str> = rows.iter().map(|r| r.key.as_str()).collect();
        let mut want: Vec<&str> = BUILTIN_TEXT_STYLES.iter().map(|(k, ..)| *k).collect();
        want.extend(["a.first", "note.big"]);
        assert_eq!(keys, want, "base order, then extending keys in key order");
        let get = |k: &str| rows.iter().find(|r| r.key.as_str() == k).unwrap();
        assert_eq!(
            (get("tag.main").em, get("tag.main").is_bold()),
            (Nm(4_000_000), true)
        );
        // An override without a weight keeps the base row's weight.
        assert_eq!(
            (get("section.id").em, get("section.id").is_bold()),
            (Nm(9_000_000), true)
        );
        assert_eq!(
            (get("note.big").em, get("note.big").is_bold()),
            (Nm(6_000_000), false)
        );
        assert!(get("a.first").is_bold());
        assert_eq!(get("text.normal").em, Nm(3_175_000));
    }

    #[test]
    fn out_of_range_sizes_and_unknown_weights_in_a_file_are_ignored() {
        let mut settings = ProjectSettings::default();
        let mut tiny = TextStyleSpec::of_size(Nm(100));
        tiny.weight = Some(TextWeight::Bold);
        settings.text_styles.insert("marker".parse().unwrap(), tiny);
        settings
            .text_styles
            .insert("note.tiny".parse().unwrap(), TextStyleSpec::of_size(Nm(-5)));
        let mut odd = TextStyleSpec::of_size(Nm(2_000_000));
        odd.weight = Some(TextWeight::Unknown(json!("black")));
        settings.text_styles.insert("section.id".parse().unwrap(), odd);
        let rows = effective_text_styles(&settings);
        assert_eq!(rows.len(), BUILTIN_TEXT_STYLES.len());
        let get = |k: &str| rows.iter().find(|r| r.key.as_str() == k).unwrap();
        assert_eq!((get("marker").em, get("marker").is_bold()), (qet_px_em(5), false));
        assert_eq!(
            (get("section.id").em, get("section.id").weight.clone()),
            (Nm(2_000_000), TextWeight::Bold)
        );
    }

    #[test]
    fn check_refuses_at_the_member_path() {
        let e = |v: Value| check_text_styles(&v).unwrap_err();
        assert_eq!(e(json!([])).pointer, "");
        assert_eq!(e(json!({" tag": {"size": 2_000_000}})).pointer, "/ tag");
        assert_eq!(e(json!({"a/b": {"size": 1}})).pointer, "/a~1b/size");
        assert_eq!(e(json!({"tag.main": 5})).pointer, "/tag.main");
        assert_eq!(e(json!({"tag.main": {}})).pointer, "/tag.main/size");
        assert_eq!(e(json!({"tag.main": {"size": "4"}})).pointer, "/tag.main/size");
        assert_eq!(
            e(json!({"tag.main": {"size": 499_999}})).pointer,
            "/tag.main/size"
        );
        assert_eq!(
            e(json!({"tag.main": {"size": 50_000_001}})).pointer,
            "/tag.main/size"
        );
        let w = e(json!({"tag.main": {"size": 4_000_000, "weight": "black"}}));
        assert_eq!(w.pointer, "/tag.main/weight");
        assert!(
            matches!(&w.problem, TextStyleProblem::Weight { weight, .. } if weight == "black"),
            "{w:?}"
        );
        let f = e(json!({"tag.main": {"size": 4_000_000, "face": "comic"}}));
        assert_eq!(f.pointer, "/tag.main/face");
        let ok = check_text_styles(&json!({
            "tag.main": {"size": 500_000, "family": "x"},
            "marker": {"size": 50_000_000, "weight": "regular", "face": "mono"}
        }))
        .unwrap();
        assert_eq!(ok["tag.main"].extra["family"], json!("x"));
        assert_eq!(ok["marker"].weight, Some(TextWeight::Regular));
        assert_eq!(ok["marker"].face, Some(Typeface::Mono));
        let settings = ProjectSettings {
            text_styles: ok,
            ..ProjectSettings::default()
        };
        let marker = effective_text_styles(&settings)
            .into_iter()
            .find(|r| r.key.as_str() == "marker")
            .unwrap();
        assert_eq!(marker.face, Typeface::Mono);
    }

    #[test]
    fn entries_round_trip_with_unknown_members_and_weights() {
        for line in [
            r#"{"size":1852083}"#,
            r#"{"size":1852083,"weight":"bold"}"#,
            r#"{"size":1852083,"weight":"black"}"#,
            r#"{"size":1852083,"weight":{"x":1},"family":"mono"}"#,
            r#"{"size":1852083,"face":"narrow"}"#,
            r#"{"size":1852083,"face":"script"}"#,
        ] {
            let spec: TextStyleSpec = serde_json::from_str(line).unwrap();
            assert_eq!(serde_json::to_string(&spec).unwrap(), line);
        }
    }
}
